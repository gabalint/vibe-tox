//! Offline recovery authorizes replacement-key enrollment, never clinical access.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Id, UserId};
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};
use secrecy::{ExposeSecret, SecretString};

use crate::account::Username;
use crate::credentials::AuthenticationMethod;
use crate::error::{Attempt, AuthError, Rejection};
use crate::key_registration::{
    KeyEnrollment, KeyRegistrationAccount, RegistrationAuthority, finish_enrollment,
};
use crate::key_secrets;
use crate::key_verifier::VerifiedKeyRegistration;
use crate::queries::session_user;
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};
use crate::repo::{self, UserStatus};
use crate::sign_in::{Failure, refuse, session_actor};
use crate::{key_repo, password, sessions, throttle};

pub(crate) enum RecoveryCode {}
pub(crate) type RecoveryCodeId = Id<RecoveryCode>;

pub(crate) fn replace_recovery_code(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
) -> Result<SecretString, AuthError> {
    let code = key_secrets::random_secret()?;
    let hash = key_secrets::hash_generated(code.clone())?;
    ctx.tx().prepare_cached(
        "UPDATE auth_key_recovery_codes SET consumed_at = ?2 WHERE user_id = ?1 AND consumed_at IS NULL",
    )?.execute(params![user_id, ctx.now()])?;
    ctx.tx().prepare_cached(
        "INSERT INTO auth_key_recovery_codes (id, user_id, code_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
    )?.execute(params![RecoveryCodeId::generate(), user_id, hash, ctx.now()])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.key_recovery_code_issued",
            Outcome::Succeeded,
        )
        .record("user", user_id),
    )?;
    Ok(code)
}

/// Rotates offline recovery material after fresh confirmation. Previous material
/// stops working immediately, and pending enrollment authorizations expire.
pub fn rotate_key_recovery_code(
    ctx: &mut CommandContext<'_>,
    confirmation: &Reauthentication,
) -> Result<Attempt<SecretString>, AuthError> {
    let (user_id, _) = session_actor(ctx)?;
    let user = repo::load(ctx.tx(), user_id)?;
    if user.authentication_method != AuthenticationMethod::SecurityKey {
        return Err(AuthError::SecurityKeyRequired);
    }
    if let Attempt::Rejected(reason) =
        reauthenticate(ctx, confirmation, ReauthPurpose::SecurityKeyRecovery)?
    {
        return Ok(Attempt::Rejected(reason));
    }
    let code = replace_recovery_code(ctx, user_id)?;
    repo::bump_revision(ctx.tx(), user_id, ctx.now())?;
    Ok(Attempt::Accepted(code))
}

/// Checks a saved recovery code and prepares only a replacement-key ceremony.
/// The code is not consumed until verified replacement commits atomically.
pub fn prepare_key_recovery(
    ctx: &mut CommandContext<'_>,
    username: &str,
    recovery_code: &SecretString,
) -> Result<Attempt<KeyRegistrationAccount>, AuthError> {
    let name = Username::parse(username).map_err(|p| p.invalid("username"))?;
    let unknown = Failure::for_username("auth.key_recovery", name.as_str());
    if unknown.is_locked(ctx)? {
        return refuse(ctx, &unknown, Rejection::LockedOut);
    }
    let Some(user) = repo::find_by_username(ctx.tx(), name.as_str())? else {
        password::verify_unknown_user(recovery_code)?;
        return refuse(ctx, &unknown, Rejection::InvalidRecoveryCode);
    };
    let mut failure = Failure::of("auth.key_recovery", &user);
    let code: Option<(RecoveryCodeId, String)> = ctx.tx().prepare_cached(
        "SELECT id, code_hash FROM auth_key_recovery_codes WHERE user_id = ?1 AND consumed_at IS NULL",
    )?.query_row([user.id], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
    let Some((code_id, hash)) = code else {
        password::verify_unknown_user(recovery_code)?;
        return refuse(ctx, &failure, Rejection::InvalidRecoveryCode);
    };
    let matches =
        recovery_code.expose_secret().len() == 64 && password::verify(recovery_code, &hash)?;
    if !matches
        || user.status != UserStatus::Active
        || user.authentication_method != AuthenticationMethod::SecurityKey
    {
        failure.reason_code = "invalid_recovery_code";
        return refuse(ctx, &failure, Rejection::InvalidRecoveryCode);
    }
    let identity = session_user(ctx.tx(), user.id)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.key_recovery_authorized",
            Outcome::Succeeded,
        )
        .record("user", user.id)
        .detail("scope", AuditDetail::code("replacement_key_only")),
    )?;
    Ok(Attempt::Accepted(KeyRegistrationAccount {
        user_id: user.id,
        username: identity.username,
        display_name: identity.display_name,
        existing: key_repo::active_keys(ctx.tx(), user.id)?,
        policy: crate::key_policy::KeyPolicy::current(ctx.tx())?,
        authority: RegistrationAuthority::Recovery {
            code_id,
            account_revision: user.revision,
        },
    }))
}

/// Atomically consumes the recovery code, revokes old keys, enrolls the verified
/// replacement, ends sessions and issues new recovery material. Opens no session.
pub fn complete_key_recovery(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyRegistration,
    label: &str,
) -> Result<KeyEnrollment, AuthError> {
    verified.account.policy.ensure_current(ctx.tx())?;
    let RegistrationAuthority::Recovery {
        code_id,
        account_revision,
    } = verified.account.authority
    else {
        return Err(AuthError::ChallengeExpired);
    };
    let user = repo::load(ctx.tx(), verified.account.user_id)?;
    user.ensure_revision(account_revision)?;
    if user.status != UserStatus::Active
        || user.authentication_method != AuthenticationMethod::SecurityKey
    {
        return Err(AuthError::ChallengeExpired);
    }
    let consumed = ctx.tx().prepare_cached(
        "UPDATE auth_key_recovery_codes SET consumed_at = ?3 WHERE id = ?1 AND user_id = ?2 AND consumed_at IS NULL",
    )?.execute(params![code_id, user.id, ctx.now()])?;
    if consumed != 1 {
        return Err(AuthError::ChallengeExpired);
    }
    key_repo::revoke_all(ctx.tx(), user.id, ctx.now())?;
    crate::unlock_pin::remove_after_reset(ctx, user.id)?;
    let enrolled = finish_enrollment(ctx, user.id, &verified.credential, label)?;
    sessions::end_sessions_of(ctx, user.id)?;
    repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    throttle::clear(ctx.tx(), user.username.as_str())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.key_recovery_completed",
            Outcome::Succeeded,
        )
        .record("user", user.id)
        .detail("scope", AuditDetail::code("replacement_key_only")),
    )?;
    Ok(enrolled)
}
