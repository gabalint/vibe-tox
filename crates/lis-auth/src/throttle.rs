//! Sign-in throttling (NFR-19): consecutive failures for a normalized username
//! lock it for a while. The laboratory's security policy sets the number of
//! failures and the duration; both are read inside the attempt's transaction,
//! so a policy change applies to the next failure. Failures are counted per
//! username whether or not an account has it, so unknown usernames behave
//! exactly like known ones. Password, unlock, password-change and MFA failures
//! all count.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AuthError;
use crate::policy::current_security_policy;

/// True while `username` is locked at `now`.
pub(crate) fn is_locked(
    conn: &Connection,
    username: &str,
    now: UtcTimestamp,
) -> Result<bool, AuthError> {
    let locked_until: Option<UtcTimestamp> = conn
        .prepare_cached("SELECT locked_until FROM auth_sign_in_failures WHERE username = ?1")?
        .query_row([username], |row| row.get(0))
        .optional()?
        .flatten();
    Ok(locked_until.is_some_and(|until| until > now))
}

/// Counts one failure and returns true when it locks the username.
pub(crate) fn record_failure(
    conn: &Connection,
    username: &str,
    now: UtcTimestamp,
) -> Result<bool, AuthError> {
    let previous: Option<(i64, Option<UtcTimestamp>)> = conn
        .prepare_cached(
            "SELECT failures, locked_until FROM auth_sign_in_failures WHERE username = ?1",
        )?
        .query_row([username], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    // An expired lock starts a new run of failures.
    let failures = match previous {
        Some((_, Some(_))) | None => 1,
        Some((count, None)) => count + 1,
    };
    let policy = current_security_policy(conn)?;
    let locked_until = if failures >= i64::from(policy.lockout_failures) {
        Some(
            now.checked_add(policy.lockout_duration())
                .ok_or(AuthError::TimeOutOfRange)?,
        )
    } else {
        None
    };
    conn.prepare_cached(
        "INSERT INTO auth_sign_in_failures (username, failures, locked_until, updated_at) \
         VALUES (?1, ?2, ?3, ?4) ON CONFLICT (username) DO UPDATE SET \
         failures = excluded.failures, locked_until = excluded.locked_until, \
         updated_at = excluded.updated_at",
    )?
    .execute(params![username, failures, locked_until, now])?;
    Ok(locked_until.is_some())
}

/// Forgets the failures of `username` after a success or a credentials reset.
pub(crate) fn clear(conn: &Connection, username: &str) -> Result<(), AuthError> {
    conn.prepare_cached("DELETE FROM auth_sign_in_failures WHERE username = ?1")?
        .execute([username])?;
    Ok(())
}
