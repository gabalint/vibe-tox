//! Encrypted PIN configuration and durable counters for existing sessions.

use lis_core::{SessionId, UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::AuthError;

pub(super) fn verifier(conn: &Connection, user: UserId) -> Result<Option<Vec<u8>>, AuthError> {
    Ok(conn
        .prepare_cached("SELECT verifier FROM auth_unlock_pins WHERE user_id = ?1")?
        .query_row([user], |row| row.get(0))
        .optional()?)
}

pub(crate) fn sealed_verifiers(conn: &Connection) -> Result<Vec<(UserId, Vec<u8>)>, AuthError> {
    Ok(conn
        .prepare_cached("SELECT user_id, verifier FROM auth_unlock_pins")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}

pub(super) fn save(
    conn: &Connection,
    user: UserId,
    sealed: &[u8],
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    conn.prepare_cached(
        "INSERT INTO auth_unlock_pins (user_id, verifier, updated_at, revision) \
        VALUES (?1, ?2, ?3, 1) ON CONFLICT(user_id) DO UPDATE SET \
        verifier = excluded.verifier, updated_at = excluded.updated_at, revision = revision + 1",
    )?
    .execute(params![user, sealed, now])?;
    Ok(())
}

pub(super) fn remove(conn: &Connection, user: UserId) -> Result<bool, AuthError> {
    Ok(conn
        .prepare_cached("DELETE FROM auth_unlock_pins WHERE user_id = ?1")?
        .execute([user])?
        != 0)
}

pub(super) fn failures(conn: &Connection, session: SessionId) -> Result<u8, AuthError> {
    Ok(conn
        .prepare_cached("SELECT failures FROM auth_unlock_pin_attempts WHERE session_id = ?1")?
        .query_row([session], |row| row.get(0))
        .optional()?
        .unwrap_or(0))
}

pub(super) fn record_failure(conn: &Connection, session: SessionId) -> Result<u8, AuthError> {
    Ok(conn
        .prepare_cached(
            "INSERT INTO auth_unlock_pin_attempts (session_id, failures) VALUES (?1, 1) \
        ON CONFLICT(session_id) DO UPDATE SET failures = min(failures + 1, 5) RETURNING failures",
        )?
        .query_row([session], |row| row.get(0))?)
}

pub(super) fn reset_failures(conn: &Connection, session: SessionId) -> Result<(), AuthError> {
    conn.prepare_cached("DELETE FROM auth_unlock_pin_attempts WHERE session_id = ?1")?
        .execute([session])?;
    Ok(())
}
