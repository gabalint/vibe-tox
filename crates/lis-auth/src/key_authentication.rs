//! Key authentication snapshots, transactional rechecks and audited failures.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{SessionId, UserId};
use lis_store::CommandContext;

use crate::account::Username;
use crate::credentials::{AuthenticationMethod, SecurityKeyId};
use crate::error::{Attempt, AuthError, Rejection};
use crate::key_policy::KeyPolicy;
use crate::key_repo::{self, StoredKey};
use crate::key_verifier::VerifiedKeyAuthentication;
use crate::queries::{SessionUser, session_user};
use crate::repo::{self, UserStatus};
use crate::sessions::{NewSession, session_is_open};
use crate::sign_in::{Failure, refuse, session_actor, start_session};
use crate::throttle;

/// The assurance required for a native assertion.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyAuthenticationPurpose {
    /// Initial sign-in or same-user unlock follows current laboratory policy.
    SignInOrUnlock,
    /// Sensitive actions always require authenticator PIN/biometric verification.
    SensitiveAction,
}

/// One account and one key, frozen before native hardware interaction.
pub struct KeyAuthenticationAccount {
    pub(crate) user_id: UserId,
    pub(crate) account_revision: i64,
    pub(crate) key: StoredKey,
    pub(crate) policy: KeyPolicy,
    pub(crate) purpose: KeyAuthenticationPurpose,
}

impl KeyAuthenticationAccount {
    /// Account identity bound to the native ceremony.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}

/// Resolves one explicitly selected active key after checking account lockout.
pub fn prepare_key_authentication(
    ctx: &mut CommandContext<'_>,
    username: &str,
    credential_id: SecurityKeyId,
) -> Result<Attempt<KeyAuthenticationAccount>, AuthError> {
    prepare_key_authentication_for_purpose(
        ctx,
        username,
        credential_id,
        KeyAuthenticationPurpose::SensitiveAction,
    )
}

/// Resolves a key and freezes the current laboratory policy for its purpose.
pub fn prepare_key_authentication_for_purpose(
    ctx: &mut CommandContext<'_>,
    username: &str,
    credential_id: SecurityKeyId,
    purpose: KeyAuthenticationPurpose,
) -> Result<Attempt<KeyAuthenticationAccount>, AuthError> {
    let Ok(name) = Username::parse(username) else {
        return record_key_failure(ctx, username).map(map_rejection);
    };
    let Some(user) = repo::find_by_username(ctx.tx(), name.as_str())? else {
        return record_key_failure(ctx, username).map(map_rejection);
    };
    let failure = Failure::of("auth.key_sign_in", &user);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    let key = key_repo::active_keys(ctx.tx(), user.id)?
        .into_iter()
        .find(|key| key.id == credential_id);
    if user.status != UserStatus::Active
        || user.authentication_method != AuthenticationMethod::SecurityKey
        || key.is_none()
    {
        return refuse(ctx, &failure, Rejection::InvalidSecurityKey);
    }
    let key = key.ok_or(AuthError::SecurityKeyUnavailable)?;
    Ok(Attempt::Accepted(KeyAuthenticationAccount {
        user_id: user.id,
        account_revision: user.revision,
        key,
        policy: KeyPolicy::current(ctx.tx())?,
        purpose,
    }))
}

fn map_rejection<T>(attempt: Attempt<()>) -> Attempt<T> {
    match attempt {
        Attempt::Rejected(reason) => Attempt::Rejected(reason),
        Attempt::Accepted(()) => Attempt::Rejected(Rejection::InvalidSecurityKey),
    }
}

/// Commits a generic failed key assertion and lockout count without logging
/// transport errors, assertion bytes, PINs or credential IDs.
pub fn record_key_failure(
    ctx: &mut CommandContext<'_>,
    username: &str,
) -> Result<Attempt<()>, AuthError> {
    let Ok(name) = Username::parse(username) else {
        ctx.audit(
            AuditEvent::new(Stream::Security, "auth.key_sign_in", Outcome::Failed)
                .detail("reason_code", AuditDetail::code("invalid_username")),
        )?;
        return Ok(Attempt::Rejected(Rejection::InvalidSecurityKey));
    };
    let user = repo::find_by_username(ctx.tx(), name.as_str())?;
    let mut failure = match user.as_ref() {
        Some(user) => Failure::of("auth.key_sign_in", user),
        None => Failure::for_username("auth.key_sign_in", name.as_str()),
    };
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    failure.reason_code = "invalid_assertion";
    refuse(ctx, &failure, Rejection::InvalidSecurityKey)
}

pub(crate) struct AcceptedKey {
    pub(crate) user_id: UserId,
    pub(crate) account_revision: i64,
    pub(crate) key_id: SecurityKeyId,
    pub(crate) key_revision: i64,
}

pub(crate) fn accept_assertion(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyAuthentication,
) -> Result<Attempt<AcceptedKey>, AuthError> {
    verified.account.policy.ensure_current(ctx.tx())?;
    let user = repo::load(ctx.tx(), verified.account.user_id)?;
    user.ensure_revision(verified.account.account_revision)?;
    let failure = Failure::of("auth.key_assertion", &user);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    if user.status != UserStatus::Active
        || user.authentication_method != AuthenticationMethod::SecurityKey
    {
        return refuse(ctx, &failure, Rejection::InvalidSecurityKey);
    }
    let key = verified.account.key;
    let mut credential = key.credential.clone();
    if credential.update_credential(&verified.result).is_none() {
        return Err(AuthError::SecurityKeyVerification);
    }
    let key_revision = key_repo::update_asserted_key(ctx.tx(), &key, &credential, ctx.now())?;
    throttle::clear(ctx.tx(), user.username.as_str())?;
    ctx.audit(
        AuditEvent::new(Stream::Security, "auth.key_assertion", Outcome::Succeeded)
            .record("user", user.id)
            .detail("security_key_id", AuditDetail::id(key.id))
            .detail(
                "user_verified",
                AuditDetail::boolean(verified.result.user_verified()),
            ),
    )?;
    Ok(Attempt::Accepted(AcceptedKey {
        user_id: user.id,
        account_revision: user.revision,
        key_id: key.id,
        key_revision,
    }))
}

/// Starts a session only after the selected registered key's assertion passes.
pub fn complete_key_sign_in(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyAuthentication,
    session: &NewSession,
) -> Result<Attempt<SessionUser>, AuthError> {
    let key = match accept_assertion(ctx, verified)? {
        Attempt::Accepted(key) => key,
        Attempt::Rejected(reason) => return Ok(Attempt::Rejected(reason)),
    };
    start_session(ctx, key.user_id, session)?;
    Ok(Attempt::Accepted(session_user(ctx.tx(), key.user_id)?))
}

/// Unlocks only the already selected person's still-open session.
pub fn complete_key_unlock(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyAuthentication,
) -> Result<Attempt<SessionUser>, AuthError> {
    let (_, session_id) = assertion_session(ctx, &verified)?;
    let key = match accept_assertion(ctx, verified)? {
        Attempt::Accepted(key) => key,
        Attempt::Rejected(reason) => return Ok(Attempt::Rejected(reason)),
    };
    ctx.audit(
        AuditEvent::new(Stream::Security, crate::actions::UNLOCK, Outcome::Succeeded)
            .record("session", session_id)
            .detail("method", AuditDetail::code("security_key")),
    )?;
    Ok(Attempt::Accepted(session_user(ctx.tx(), key.user_id)?))
}

/// The assertion can confirm only its own still-open acting session.
pub(crate) fn assertion_session(
    ctx: &CommandContext<'_>,
    verified: &VerifiedKeyAuthentication,
) -> Result<(UserId, SessionId), AuthError> {
    let (user_id, session_id) = session_actor(ctx)?;
    if verified.account.user_id != user_id || !session_is_open(ctx.tx(), session_id, user_id)? {
        return Err(AuthError::ChallengeExpired);
    }
    Ok((user_id, session_id))
}
