//! Optional six-digit, same-session unlock (REQ-IDN-004, NFR-19).
//! The engine gates locked/unlocked state; this domain verifies the open session,
//! account, encrypted verifier and durable attempt budget inside the transaction.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{SessionId, UserId};
use lis_store::CommandContext;
use rusqlite::Connection;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zeroize::Zeroizing;

use crate::cipher::{self, DataKey, SecretPurpose};
use crate::error::{Attempt, AuthError, Problem, Rejection};
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};
use crate::sign_in::session_actor;
use crate::{SessionUser, actions, password, queries, repo, sessions, unlock_pin_repo as pins};

const ATTEMPTS: u8 = 5;

/// Sets or replaces the signed-in person's optional unlock PIN.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetUnlockPinInput {
    /// Exactly six ASCII digits, excluding the documented common patterns.
    #[ts(type = "string")]
    pub pin: SecretString,
    /// Fresh password or trusted security-key confirmation, never the old PIN.
    pub password: Reauthentication,
}

/// Removes the signed-in person's unlock PIN after normal authentication.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DisableUnlockPinInput {
    pub password: Reauthentication,
}

/// Attempts to unlock the existing same-user session; cannot create a session.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UnlockWithPinInput {
    #[ts(type = "string")]
    pub pin: SecretString,
}

/// Public-to-the-current-session configuration; no verifier or credential bytes.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UnlockPinStatus {
    pub enabled: bool,
    /// Zero when disabled; otherwise the remaining budget for this session.
    pub attempts_remaining: u8,
}

fn ensure_session(
    conn: &Connection,
    user_id: UserId,
    session_id: SessionId,
) -> Result<repo::UserRow, AuthError> {
    let user = repo::load(conn, user_id)?;
    if user.status != repo::UserStatus::Active
        || user.must_change_password
        || !sessions::session_is_open(conn, session_id, user_id)?
    {
        return Err(AuthError::SessionUnavailable);
    }
    Ok(user)
}

/// Reads only the engine-selected user's existing session, including while locked.
pub fn get_unlock_pin_status(
    conn: &Connection,
    user_id: UserId,
    session_id: SessionId,
) -> Result<UnlockPinStatus, AuthError> {
    ensure_session(conn, user_id, session_id)?;
    let enabled = pins::verifier(conn, user_id)?.is_some();
    Ok(UnlockPinStatus {
        enabled,
        attempts_remaining: if enabled {
            ATTEMPTS.saturating_sub(pins::failures(conn, session_id)?)
        } else {
            0
        },
    })
}

/// Requires an unlocked session at the engine boundary and fresh normal authentication.
pub fn set_unlock_pin(
    ctx: &mut CommandContext<'_>,
    key: &DataKey,
    input: SetUnlockPinInput,
) -> Result<Attempt<()>, AuthError> {
    let (user_id, session_id) = session_actor(ctx).map_err(|_| AuthError::SessionUnavailable)?;
    let user = ensure_session(ctx.tx(), user_id, session_id)?;
    validate_new_pin(&input.pin)?;
    if let Attempt::Rejected(reason) =
        reauthenticate(ctx, &input.password, ReauthPurpose::UnlockPinChange)?
    {
        return Ok(Attempt::Rejected(reason));
    }
    let hash = Zeroizing::new(password::hash_secret(&input.pin)?);
    let sealed = cipher::seal(key, user_id, SecretPurpose::UnlockPin, hash.as_bytes())?;
    pins::save(ctx.tx(), user_id, &sealed, ctx.now())?;
    finish_change(ctx, &user, session_id, actions::UNLOCK_PIN_SET)?;
    Ok(Attempt::Accepted(()))
}

/// Removes the PIN; changing this setting never accepts the PIN itself as proof.
pub fn disable_unlock_pin(
    ctx: &mut CommandContext<'_>,
    input: DisableUnlockPinInput,
) -> Result<Attempt<()>, AuthError> {
    let (user_id, session_id) = session_actor(ctx).map_err(|_| AuthError::SessionUnavailable)?;
    let user = ensure_session(ctx.tx(), user_id, session_id)?;
    if let Attempt::Rejected(reason) =
        reauthenticate(ctx, &input.password, ReauthPurpose::UnlockPinChange)?
    {
        return Ok(Attempt::Rejected(reason));
    }
    pins::remove(ctx.tx(), user_id)?;
    finish_change(ctx, &user, session_id, actions::UNLOCK_PIN_DISABLED)?;
    Ok(Attempt::Accepted(()))
}

fn finish_change(
    ctx: &mut CommandContext<'_>,
    user: &repo::UserRow,
    session_id: SessionId,
    action: &'static str,
) -> Result<(), AuthError> {
    pins::reset_failures(ctx.tx(), session_id)?;
    sessions::end_other_sessions(ctx, user.id, Some(session_id))?;
    let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    ctx.audit(
        AuditEvent::new(Stream::Security, action, Outcome::Succeeded)
            .record("user", user.id)
            .old_revision(user.revision)
            .new_revision(revision)
            .detail("session_id", AuditDetail::id(session_id)),
    )?;
    Ok(())
}

/// Credential reset/recovery removes convenience unlock in the same transaction.
pub(crate) fn remove_after_reset(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
) -> Result<(), AuthError> {
    if pins::remove(ctx.tx(), user_id)? {
        ctx.audit(
            AuditEvent::new(
                Stream::Security,
                actions::UNLOCK_PIN_DISABLED,
                Outcome::Succeeded,
            )
            .record("user", user_id)
            .detail("reason", AuditDetail::code("credentials_reset")),
        )?;
    }
    Ok(())
}

/// Verifies only the principal's existing session. Five incorrect or malformed
/// attempts expire it; only normal sign-in can establish another session.
pub fn unlock_with_pin(
    ctx: &mut CommandContext<'_>,
    key: &DataKey,
    pin: &SecretString,
) -> Result<Attempt<SessionUser>, AuthError> {
    let (user_id, session_id) = session_actor(ctx).map_err(|_| AuthError::SessionUnavailable)?;
    ensure_session(ctx.tx(), user_id, session_id)?;
    let sealed = pins::verifier(ctx.tx(), user_id)?.ok_or(AuthError::UnlockPinUnavailable)?;
    let hash = cipher::open(key, user_id, SecretPurpose::UnlockPin, &sealed)?;
    let hash = std::str::from_utf8(&hash).map_err(|_| AuthError::SecretCipher)?;
    if !valid_format(pin.expose_secret()) || !password::verify(pin, hash)? {
        return refuse_pin(ctx, session_id);
    }
    pins::reset_failures(ctx.tx(), session_id)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            actions::UNLOCK_WITH_PIN,
            Outcome::Succeeded,
        )
        .record("session", session_id),
    )?;
    Ok(Attempt::Accepted(queries::session_user(ctx.tx(), user_id)?))
}

fn refuse_pin(
    ctx: &mut CommandContext<'_>,
    session_id: SessionId,
) -> Result<Attempt<SessionUser>, AuthError> {
    let failures = pins::record_failure(ctx.tx(), session_id)?;
    let exhausted = failures >= ATTEMPTS;
    ctx.audit(
        AuditEvent::new(Stream::Security, actions::UNLOCK_WITH_PIN, Outcome::Failed)
            .record("session", session_id)
            .detail(
                "reason",
                AuditDetail::code(if exhausted {
                    "attempts_exhausted"
                } else {
                    "incorrect"
                }),
            )
            .detail(
                "attempts_remaining",
                AuditDetail::integer(i64::from(ATTEMPTS.saturating_sub(failures))),
            ),
    )?;
    if exhausted {
        sessions::end_session(ctx, session_id, sessions::EndReason::Expired)?;
    }
    Ok(Attempt::Rejected(if exhausted {
        Rejection::UnlockPinAttemptsExhausted
    } else {
        Rejection::WrongUnlockPin
    }))
}

fn valid_format(pin: &str) -> bool {
    pin.len() == 6 && pin.bytes().all(|byte| byte.is_ascii_digit())
}

/// Reject identical digits, ascending/descending runs (including wrap), repeated
/// two/three-digit blocks and this fixed common-pattern list. No dates are inferred.
fn validate_new_pin(pin: &SecretString) -> Result<(), AuthError> {
    let text = pin.expose_secret();
    if !valid_format(text) {
        return Err(Problem {
            code: "format",
            message: "Enter exactly six digits (0–9).",
        }
        .invalid("pin"));
    }
    let bytes = text.as_bytes();
    let sequential = bytes
        .windows(2)
        .all(|pair| (pair[0] - b'0' + 1) % 10 == pair[1] - b'0')
        || bytes
            .windows(2)
            .all(|pair| (pair[0] - b'0' + 9) % 10 == pair[1] - b'0');
    let repeated = [1, 2, 3].iter().any(|length| {
        bytes
            .iter()
            .enumerate()
            .all(|(i, byte)| *byte == bytes[i % length])
    });
    if sequential
        || repeated
        || ["112233", "123321", "159753", "147258", "258369", "102030"].contains(&text)
    {
        return Err(Problem { code: "common", message: "Choose a less predictable PIN without common sequences or repeating patterns." }.invalid("pin"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_ascii_digits_exclude_documented_common_patterns() {
        for pin in [
            "",
            "12345",
            "1234567",
            "12 456",
            "１２３４５６",
            "111111",
            "123456",
            "987654",
            "890123",
            "210987",
            "121212",
            "123123",
            "112233",
            "123321",
            "159753",
            "147258",
            "258369",
            "102030",
        ] {
            assert!(validate_new_pin(&SecretString::from(pin)).is_err(), "{pin}");
        }
        assert!(validate_new_pin(&SecretString::from("482905")).is_ok());
    }
}
