//! Single-use receipts for the engine's frozen sensitive-action gateway.
//! Receipts are never sent to the UI; existing domain commands consume one
//! inside the action transaction instead of checking a password (NFR-19).

use std::time::Duration;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};
use secrecy::SecretString;

use crate::credentials::AuthenticationMethod;
use crate::error::{Attempt, AuthError, Rejection};
use crate::key_authentication::{accept_assertion, assertion_session};
use crate::key_secrets;
use crate::key_verifier::VerifiedKeyAuthentication;
use crate::reauthenticate::{ReauthPurpose, Reauthentication};
use crate::repo::{UserRow, UserStatus};
use crate::sessions::session_is_open;
use crate::sign_in::{Failure, refuse, session_actor};

/// Creates an action receipt only from a verified assertion for the acting
/// session. The caller owns the exact frozen action and must not expose the
/// receipt or accept replacement action arguments after this point.
pub fn issue_key_action_proof(
    ctx: &mut CommandContext<'_>,
    verified: VerifiedKeyAuthentication,
    purpose: ReauthPurpose,
) -> Result<Attempt<Reauthentication>, AuthError> {
    if !verified.result.user_verified() {
        return Err(AuthError::SecurityKeyVerification);
    }
    let (user_id, session_id) = assertion_session(ctx, &verified)?;
    let key = match accept_assertion(ctx, verified)? {
        Attempt::Accepted(key) => key,
        Attempt::Rejected(reason) => return Ok(Attempt::Rejected(reason)),
    };
    let secret = key_secrets::random_secret()?;
    let expires = ctx
        .now()
        .checked_add(Duration::from_secs(60))
        .ok_or(AuthError::TimeOutOfRange)?;
    ctx.tx()
        .prepare_cached(
            "DELETE FROM auth_key_action_proofs WHERE expires_at < ?1 OR consumed_at IS NOT NULL",
        )?
        .execute([ctx.now()])?;
    ctx.tx()
        .prepare_cached(
            "INSERT INTO auth_key_action_proofs (proof_digest, user_id, session_id, key_id, \
         purpose, account_revision, key_revision, issued_at, expires_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?
        .execute(params![
            key_secrets::digest(&secret),
            user_id,
            session_id,
            key.key_id,
            purpose.code(),
            key.account_revision,
            key.key_revision,
            ctx.now(),
            expires
        ])?;
    Ok(Attempt::Accepted(Reauthentication::SecurityKey {
        key_proof: secret,
    }))
}

pub(crate) fn consume_key_action_proof(
    ctx: &mut CommandContext<'_>,
    user: &UserRow,
    secret: &SecretString,
    purpose: ReauthPurpose,
) -> Result<Attempt<()>, AuthError> {
    let (_, session_id) = session_actor(ctx)?;
    let mut failure = Failure::of(crate::actions::REAUTHENTICATE, user);
    failure.purpose = Some(purpose);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    let valid_account = user.status == UserStatus::Active
        && user.authentication_method == AuthenticationMethod::SecurityKey
        && session_is_open(ctx.tx(), session_id, user.id)?;
    let digest = key_secrets::digest(secret);
    let consumed = if valid_account {
        consume_matching(ctx, user, session_id, &digest, purpose)?
    } else {
        false
    };
    if !consumed {
        failure.reason_code = "invalid_key_confirmation";
        return refuse(ctx, &failure, Rejection::InvalidSecurityKey);
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            crate::actions::REAUTHENTICATE,
            Outcome::Succeeded,
        )
        .record("user", user.id)
        .detail("session_id", AuditDetail::id(session_id))
        .detail("purpose", AuditDetail::code(purpose.code()))
        .detail("method", AuditDetail::code("security_key")),
    )?;
    Ok(Attempt::Accepted(()))
}

fn consume_matching(
    ctx: &CommandContext<'_>,
    user: &UserRow,
    session_id: lis_core::SessionId,
    digest: &[u8],
    purpose: ReauthPurpose,
) -> Result<bool, AuthError> {
    Ok(ctx
        .tx()
        .prepare_cached(
            "UPDATE auth_key_action_proofs SET consumed_at = ?6 \
         WHERE proof_digest = ?1 AND user_id = ?2 AND session_id = ?3 \
         AND purpose = ?4 AND account_revision = ?5 AND consumed_at IS NULL \
         AND issued_at <= ?6 AND expires_at > ?6 AND EXISTS (\
           SELECT 1 FROM auth_security_keys k WHERE k.id = auth_key_action_proofs.key_id \
           AND k.user_id = ?2 AND k.revision = auth_key_action_proofs.key_revision \
           AND k.revoked_at IS NULL) RETURNING 1",
        )?
        .query_row(
            params![
                digest,
                user.id,
                session_id,
                purpose.code(),
                user.revision,
                ctx.now()
            ],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}
