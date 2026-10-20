//! Authentication commands: password step, temporary-password replacement,
//! second factor, unlock and password change (REQ-IDN-004, NFR-19).
//!
//! An attempt that fails still commits: it returns [`Attempt::Rejected`] after
//! counting the failure and auditing it, so the lockout cannot be bypassed by
//! a rollback. No password, code, secret or hash ever enters an audit event.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Actor, SessionId, UserId};
use lis_store::CommandContext;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use ts_rs::TS;

use crate::account::Username;
use crate::actions;
use crate::challenge::{ChallengeId, ChallengeStep, LiveChallenge, PasswordAccepted};
use crate::cipher::{self, DataKey};
use crate::credentials::AuthenticationMethod;
use crate::error::{Attempt, AuthError, Problem, Rejection};
use crate::password::{self, NewPassword};
use crate::queries::{self, SessionUser};
use crate::repo::{self, UserRow, UserStatus};
use crate::sessions::{self, EndReason, NewSession};
pub(crate) use crate::sign_in_failure::{Failure, refuse};
use crate::throttle;
use crate::totp::TotpSecret;

/// Input of `sign_in`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SignInInput {
    /// As typed; matched case-insensitively.
    pub username: String,
    /// The password.
    #[ts(type = "string")]
    pub password: SecretString,
}

/// Input of `change_initial_password`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ChangeInitialPasswordInput {
    /// The live password-change challenge.
    pub challenge_id: ChallengeId,
    /// The new password.
    #[ts(type = "string")]
    pub new_password: SecretString,
}

/// Input of `verify_mfa`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct VerifyMfaInput {
    /// The live MFA or enrolment challenge.
    pub challenge_id: ChallengeId,
    /// The 6-digit authenticator code.
    #[ts(type = "string")]
    pub code: SecretString,
}

/// Input of `unlock_session`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UnlockSessionInput {
    /// The session user's password.
    #[ts(type = "string")]
    pub password: SecretString,
}

/// Input of `change_password`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ChangePasswordInput {
    /// The password in use now.
    #[ts(type = "string")]
    pub current_password: SecretString,
    /// The new password.
    #[ts(type = "string")]
    pub new_password: SecretString,
}

/// Why `password` does not admit `user`, as an audit reason code; `None` if it does.
pub(crate) fn password_problem(
    user: &UserRow,
    password: &SecretString,
) -> Result<Option<&'static str>, AuthError> {
    if user.authentication_method == AuthenticationMethod::SecurityKey {
        return Ok(Some("security_key_required"));
    }
    if !password::verify(password, &user.password_hash)? {
        return Ok(Some("wrong_password"));
    }
    Ok((user.status == UserStatus::Disabled).then_some("disabled"))
}

/// The password step of `sign_in`. Unknown usernames, wrong passwords and
/// disabled accounts are refused identically and spend the same hashing work.
pub fn check_password(
    ctx: &mut CommandContext<'_>,
    input: &SignInInput,
) -> Result<Attempt<PasswordAccepted>, AuthError> {
    let Ok(username) = Username::parse(&input.username) else {
        // Not a possible username: nothing to count, and nothing safe to record.
        ctx.audit(
            AuditEvent::new(Stream::Security, actions::SIGN_IN, Outcome::Failed)
                .detail("reason_code", AuditDetail::code("invalid_username")),
        )?;
        return Ok(Attempt::Rejected(Rejection::InvalidCredentials));
    };
    let user = repo::find_by_username(ctx.tx(), username.as_str())?;
    let mut failure = match user.as_ref() {
        Some(user) => Failure::of(actions::SIGN_IN, user),
        None => Failure::for_username(actions::SIGN_IN, username.as_str()),
    };
    failure.reason_code = "locked_out";
    if throttle::is_locked(ctx.tx(), username.as_str(), ctx.now())? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    let Some(user) = user.as_ref() else {
        password::verify_unknown_user(&input.password)?;
        failure.reason_code = "unknown_user";
        return refuse(ctx, &failure, Rejection::InvalidCredentials);
    };
    if let Some(reason_code) = password_problem(user, &input.password)? {
        failure.reason_code = reason_code;
        return refuse(ctx, &failure, Rejection::InvalidCredentials);
    }
    Ok(Attempt::Accepted(accepted(user)))
}

fn accepted(user: &UserRow) -> PasswordAccepted {
    PasswordAccepted {
        user_id: user.id,
        account_revision: user.revision,
        username: user.username.clone(),
        must_change_password: user.must_change_password,
        mfa_enrolled: user.totp_secret.is_some(),
    }
}

/// Replaces a temporary password during sign-in; the challenge authorizes it.
pub fn set_initial_password(
    ctx: &mut CommandContext<'_>,
    challenge: &LiveChallenge,
    new_password: SecretString,
) -> Result<PasswordAccepted, AuthError> {
    let user = repo::load(ctx.tx(), challenge.user_id)?;
    if !matches!(challenge.step, ChallengeStep::PasswordChange)
        || user.revision != challenge.account_revision
        || user.status != UserStatus::Active
        || !user.must_change_password
    {
        return Err(AuthError::ChallengeExpired);
    }
    let new = NewPassword::parse(new_password, user.username.as_str())
        .map_err(|problem| problem.invalid("newPassword"))?;
    if password::verify(new.secret(), &user.password_hash)? {
        return Err(Problem {
            code: "unchanged",
            message: "Choose a password different from the temporary one.",
        }
        .invalid("newPassword"));
    }
    repo::set_password(ctx.tx(), user.id, &password::hash(&new)?, false)?;
    let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            actions::PASSWORD_CHANGE,
            Outcome::Succeeded,
        )
        .record("user", user.id)
        .old_revision(user.revision)
        .new_revision(revision)
        .detail("initial", AuditDetail::boolean(true)),
    )?;
    let mut next = accepted(&user);
    next.account_revision = revision;
    next.must_change_password = false;
    Ok(next)
}

/// The second-factor step: checks the code (confirming a pending enrolment)
/// and starts the session, ending the session it replaces.
pub fn complete_sign_in(
    ctx: &mut CommandContext<'_>,
    key: &DataKey,
    challenge: &LiveChallenge,
    code: &SecretString,
    session: &NewSession,
) -> Result<Attempt<SessionUser>, AuthError> {
    let user = repo::load(ctx.tx(), challenge.user_id)?;
    // NFR-19: a queued copy of a challenge cannot outlive a credential reset.
    if user.revision != challenge.account_revision || user.must_change_password {
        return Err(AuthError::ChallengeExpired);
    }
    let mut failure = Failure::of(actions::MFA_VERIFY, &user);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    if user.status == UserStatus::Disabled {
        failure.reason_code = "disabled";
        return refuse(ctx, &failure, Rejection::InvalidCredentials);
    }
    let (secret, enrolling) = factor_secret(key, challenge, &user)?;
    let last_step = if enrolling { None } else { user.totp_last_step };
    let step = match secret.matching_step(code.expose_secret(), ctx.now())? {
        Some(step) if last_step.is_none_or(|last| step > last) => step,
        matched => {
            // RFC 6238 section 5.2: an accepted step is never accepted again.
            failure.reason_code = if matched.is_some() {
                "replayed_code"
            } else {
                "invalid_code"
            };
            return refuse(ctx, &failure, Rejection::InvalidCode);
        }
    };
    if enrolling {
        let sealed = cipher::seal(key, user.id, cipher::SecretPurpose::Totp, secret.as_bytes())?;
        repo::set_totp(ctx.tx(), user.id, Some((&sealed, step)))?;
        let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
        ctx.audit(
            AuditEvent::new(Stream::Security, actions::MFA_ENROLL, Outcome::Succeeded)
                .record("user", user.id)
                .old_revision(user.revision)
                .new_revision(revision),
        )?;
    } else {
        repo::set_totp_step(ctx.tx(), user.id, step)?;
    }
    throttle::clear(ctx.tx(), user.username.as_str())?;
    start_session(ctx, user.id, session)?;
    Ok(Attempt::Accepted(queries::session_user(ctx.tx(), user.id)?))
}

/// The secret the code must match, and whether it is a pending enrolment.
fn factor_secret(
    key: &DataKey,
    challenge: &LiveChallenge,
    user: &UserRow,
) -> Result<(TotpSecret, bool), AuthError> {
    match (&challenge.step, &user.totp_secret) {
        (ChallengeStep::Enrollment(pending), None) => Ok((pending.0.clone(), true)),
        (ChallengeStep::Mfa, Some(sealed)) => Ok((
            TotpSecret::from_bytes(cipher::open(
                key,
                user.id,
                cipher::SecretPurpose::Totp,
                sealed,
            )?),
            false,
        )),
        // The account changed since the challenge was issued.
        _ => Err(AuthError::ChallengeExpired),
    }
}

pub(crate) fn start_session(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
    session: &NewSession,
) -> Result<(), AuthError> {
    if let Some(previous) = session.replaces {
        sessions::end_session(ctx, previous, EndReason::Switched)?;
    }
    sessions::insert_session(ctx.tx(), session, user_id, ctx.now())?;
    let event = AuditEvent::new(Stream::Security, actions::SIGN_IN, Outcome::Succeeded)
        .record("user", user_id)
        .detail("session_id", AuditDetail::id(session.id))
        .detail("workstation_id", AuditDetail::id(session.workstation_id));
    ctx.audit(event)?;
    Ok(())
}

/// The signed-in user and session of the principal running the command.
pub(crate) fn session_actor(ctx: &CommandContext<'_>) -> Result<(UserId, SessionId), AuthError> {
    match ctx.principal().actor() {
        Actor::User {
            user_id,
            session_id,
            ..
        } => Ok((*user_id, *session_id)),
        Actor::Instrument { .. } | Actor::System { .. } => Err(AuthError::UserNotFound),
    }
}

/// Unlocks the principal's locked session with the same user's password.
pub fn unlock(
    ctx: &mut CommandContext<'_>,
    password: &SecretString,
) -> Result<Attempt<SessionUser>, AuthError> {
    let (user_id, session_id) = session_actor(ctx)?;
    let user = repo::load(ctx.tx(), user_id)?;
    let mut failure = Failure::of(actions::UNLOCK, &user);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    if let Some(reason_code) = password_problem(&user, password)? {
        failure.reason_code = reason_code;
        return refuse(ctx, &failure, Rejection::InvalidCredentials);
    }
    throttle::clear(ctx.tx(), user.username.as_str())?;
    ctx.audit(
        AuditEvent::new(Stream::Security, actions::UNLOCK, Outcome::Succeeded)
            .record("session", session_id),
    )?;
    Ok(Attempt::Accepted(queries::session_user(ctx.tx(), user.id)?))
}

/// Changes the principal's own password after checking the current one.
pub fn change_password(
    ctx: &mut CommandContext<'_>,
    input: ChangePasswordInput,
) -> Result<Attempt<()>, AuthError> {
    let (user_id, _) = session_actor(ctx)?;
    let user = repo::load(ctx.tx(), user_id)?;
    if user.authentication_method == AuthenticationMethod::SecurityKey {
        return Err(AuthError::SecurityKeyRequired);
    }
    let new = NewPassword::parse(input.new_password, user.username.as_str())
        .map_err(|problem| problem.invalid("newPassword"))?;
    let mut failure = Failure::of(actions::PASSWORD_CHANGE, &user);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    if !password::verify(&input.current_password, &user.password_hash)? {
        failure.reason_code = "wrong_password";
        return refuse(ctx, &failure, Rejection::WrongCurrentPassword);
    }
    repo::set_password(ctx.tx(), user.id, &password::hash(&new)?, false)?;
    let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    throttle::clear(ctx.tx(), user.username.as_str())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            actions::PASSWORD_CHANGE,
            Outcome::Succeeded,
        )
        .record("user", user.id)
        .old_revision(user.revision)
        .new_revision(revision),
    )?;
    Ok(Attempt::Accepted(()))
}
