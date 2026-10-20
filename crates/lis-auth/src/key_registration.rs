//! Atomic first-account enrollment and authorized migration/spare-key enrollment.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{SessionId, UserId};
use lis_store::CommandContext;
use rusqlite::Connection;
use secrecy::SecretString;
use serde::Deserialize;
use ts_rs::TS;

use crate::account::{Username, parse_display_name};
use crate::admin::{ValidAccount, account_created_event, insert_account_at};
use crate::credentials::AuthenticationMethod;
use crate::error::{Attempt, AuthError, Problem};
use crate::key_credential::KeyCredential;
use crate::key_policy::KeyPolicy;
use crate::key_recovery::{RecoveryCodeId, replace_recovery_code};
use crate::key_repo::{self, StoredKey};
use crate::key_verifier::VerifiedKeyRegistration;
use crate::one_person::{allow_first_run_account, first_run_roles};
use crate::queries::{SessionUser, session_user};
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};
use crate::repo::{self, UserStatus};
use crate::sessions::{self, NewSession};
use crate::sign_in::{session_actor, start_session};

/// First-run account details for direct key registration; there is no password.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FirstKeyAdministratorInput {
    /// The individual account's sign-in name.
    pub username: String,
    /// The name shown in the application and audit trail.
    pub display_name: String,
    /// Assign every role for a laboratory operated by one person.
    pub one_person: bool,
    /// Only the engine's separate demo installation may enable this choice.
    #[serde(default)]
    #[ts(optional)]
    pub load_demo_data: Option<bool>,
}

pub(crate) enum RegistrationAuthority {
    First {
        one_person: bool,
        demo_setup: bool,
    },
    Own {
        session_id: SessionId,
        account_revision: i64,
    },
    Recovery {
        code_id: RecoveryCodeId,
        account_revision: i64,
    },
}

/// Validated identity and authorization retained in Rust during enrollment.
pub struct KeyRegistrationAccount {
    pub(crate) user_id: UserId,
    pub(crate) username: String,
    pub(crate) display_name: String,
    pub(crate) existing: Vec<StoredKey>,
    pub(crate) authority: RegistrationAuthority,
    pub(crate) policy: KeyPolicy,
}

impl KeyRegistrationAccount {
    /// The account ID bound to the authenticator credential.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}

/// One-time recovery material plus a private, first-setup-only session permit.
pub struct KeyEnrollment {
    user_id: UserId,
    recovery_code: SecretString,
    first_session: Option<(crate::SecurityKeyId, i64)>,
}

impl KeyEnrollment {
    /// Account whose enrollment completed.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    /// Display once, then keep only in the operator's offline custody.
    #[must_use]
    pub fn recovery_code(&self) -> &SecretString {
        &self.recovery_code
    }
}

/// Validates first-run identity before starting the native prompt. No account
/// exists until successful verification commits in `complete_first_key_registration`.
pub fn prepare_first_key_registration(
    conn: &Connection,
    input: FirstKeyAdministratorInput,
) -> Result<KeyRegistrationAccount, AuthError> {
    if repo::any_user(conn)? {
        return Err(AuthError::AlreadySetUp);
    }
    let username = Username::parse(&input.username).map_err(|p| p.invalid("username"))?;
    let display_name =
        parse_display_name(&input.display_name).map_err(|p| p.invalid("displayName"))?;
    let demo_setup = crate::setup::validate_setup_options(input.one_person, input.load_demo_data)?;
    Ok(KeyRegistrationAccount {
        user_id: UserId::generate(),
        username: username.to_string(),
        display_name,
        existing: Vec::new(),
        policy: KeyPolicy::current(conn)?,
        authority: RegistrationAuthority::First {
            one_person: input.one_person,
            demo_setup,
        },
    })
}

/// Authorizes one's own migration or spare registration with fresh confirmation.
pub fn prepare_own_key_registration(
    ctx: &mut CommandContext<'_>,
    confirmation: &Reauthentication,
) -> Result<Attempt<KeyRegistrationAccount>, AuthError> {
    let (user_id, session_id) = session_actor(ctx)?;
    if !sessions::session_is_open(ctx.tx(), session_id, user_id)? {
        return Err(AuthError::ChallengeExpired);
    }
    if let Attempt::Rejected(reason) =
        reauthenticate(ctx, confirmation, ReauthPurpose::SecurityKeyEnrollment)?
    {
        return Ok(Attempt::Rejected(reason));
    }
    let user = repo::load(ctx.tx(), user_id)?;
    let identity = session_user(ctx.tx(), user_id)?;
    Ok(Attempt::Accepted(KeyRegistrationAccount {
        user_id,
        username: identity.username,
        display_name: identity.display_name,
        existing: key_repo::active_keys(ctx.tx(), user_id)?,
        policy: KeyPolicy::current(ctx.tx())?,
        authority: RegistrationAuthority::Own {
            session_id,
            account_revision: user.revision,
        },
    }))
}

/// Creates the first account, grants its roles and stores its verified key and
/// hashed recovery code in one transaction; never opens a session by itself.
pub fn complete_first_key_registration(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyRegistration,
    label: &str,
) -> Result<KeyEnrollment, AuthError> {
    verified.account.policy.ensure_current(ctx.tx())?;
    let RegistrationAuthority::First {
        one_person,
        demo_setup,
    } = verified.account.authority
    else {
        return Err(AuthError::ChallengeExpired);
    };
    if repo::any_user(ctx.tx())? {
        return Err(AuthError::AlreadySetUp);
    }
    let account = ValidAccount {
        username: Username::parse(&verified.account.username).map_err(|p| p.invalid("username"))?,
        display_name: verified.account.display_name,
        password_hash: crate::key_secrets::hash_generated(crate::key_secrets::random_secret()?)?,
    };
    let user_id = verified.account.user_id;
    let roles = first_run_roles(one_person);
    insert_account_at(ctx, &account, &roles, false, user_id)?;
    key_repo::use_key_method(ctx.tx(), user_id)?;
    ctx.audit(account_created_event(
        crate::actions::FIRST_ADMINISTRATOR_CREATE,
        user_id,
        &account,
        &roles,
    ))?;
    if one_person {
        allow_first_run_account(ctx)?;
    }
    if demo_setup {
        request_demo(ctx, user_id)?;
    }
    let mut enrolled = finish_enrollment(ctx, user_id, &verified.credential, label)?;
    let key = key_repo::active_keys(ctx.tx(), user_id)?
        .into_iter()
        .next()
        .ok_or(AuthError::SecurityKeyUnavailable)?;
    enrolled.first_session = Some((key.id, 1));
    Ok(enrolled)
}

fn request_demo(ctx: &CommandContext<'_>, user_id: UserId) -> Result<(), AuthError> {
    repo::request_demo_setup(ctx.tx(), user_id, ctx.now())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.demo_setup_requested",
            Outcome::Succeeded,
        )
        .record("user", user_id),
    )?;
    Ok(())
}

/// Commits a spare key or migration. A migration retires password/TOTP access;
/// the currently authenticated session remains, but other sessions end.
pub fn complete_own_key_registration(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyRegistration,
    label: &str,
) -> Result<KeyEnrollment, AuthError> {
    verified.account.policy.ensure_current(ctx.tx())?;
    let RegistrationAuthority::Own {
        session_id,
        account_revision,
    } = verified.account.authority
    else {
        return Err(AuthError::ChallengeExpired);
    };
    let (user_id, current_session) = session_actor(ctx)?;
    if user_id != verified.account.user_id
        || current_session != session_id
        || !sessions::session_is_open(ctx.tx(), session_id, user_id)?
    {
        return Err(AuthError::ChallengeExpired);
    }
    let user = repo::load(ctx.tx(), user_id)?;
    user.ensure_revision(account_revision)?;
    if user.status != UserStatus::Active {
        return Err(AuthError::ChallengeExpired);
    }
    let enrolled = finish_enrollment(ctx, user_id, &verified.credential, label)?;
    if user.authentication_method == AuthenticationMethod::PasswordTotp {
        key_repo::use_key_method(ctx.tx(), user_id)?;
        sessions::end_other_sessions(ctx, user_id, Some(session_id))?;
    }
    repo::bump_revision(ctx.tx(), user_id, ctx.now())?;
    Ok(enrolled)
}

pub(crate) fn finish_enrollment(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
    credential: &KeyCredential,
    label: &str,
) -> Result<KeyEnrollment, AuthError> {
    let label = validate_key_label(label)?;
    let key_id = key_repo::insert_key(ctx.tx(), user_id, credential, &label, ctx.now())?;
    let recovery_code = replace_recovery_code(ctx, user_id)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.security_key_enrolled",
            Outcome::Succeeded,
        )
        .record("user", user_id)
        .detail("security_key_id", AuditDetail::id(key_id)),
    )?;
    Ok(KeyEnrollment {
        user_id,
        recovery_code,
        first_session: None,
    })
}

fn validate_key_label(label: &str) -> Result<String, AuthError> {
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 80 || label.chars().any(char::is_control) {
        return Err(Problem {
            code: "invalid",
            message: "Enter a security-key name of 1 to 80 characters.",
        }
        .invalid("keyLabel"));
    }
    Ok(label.to_owned())
}

/// Consumes a first-setup-only permit after the operator acknowledges custody of
/// the recovery code. Recovery and later enrollment can never use this path.
pub fn complete_key_enrollment_session(
    ctx: &mut CommandContext<'_>,
    enrollment: KeyEnrollment,
    session: &NewSession,
) -> Result<SessionUser, AuthError> {
    let (key_id, account_revision) = enrollment
        .first_session
        .ok_or(AuthError::ChallengeExpired)?;
    let user = repo::load(ctx.tx(), enrollment.user_id)?;
    user.ensure_revision(account_revision)?;
    if user.status != UserStatus::Active
        || user.authentication_method != AuthenticationMethod::SecurityKey
        || !key_repo::active_keys(ctx.tx(), user.id)?
            .iter()
            .any(|key| key.id == key_id && key.revision == 1)
    {
        return Err(AuthError::ChallengeExpired);
    }
    start_session(ctx, user.id, session)?;
    session_user(ctx.tx(), user.id)
}
