//! SQL for registered security keys. Only verified library credentials enter it.

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::credentials::SecurityKeyId;
use crate::error::AuthError;
use crate::key_credential::KeyCredential;

pub(crate) struct StoredKey {
    pub(crate) id: SecurityKeyId,
    pub(crate) user_id: UserId,
    pub(crate) credential: KeyCredential,
    pub(crate) revision: i64,
    pub(crate) assertion_revision: i64,
}

pub(crate) fn credential_id(credential: &KeyCredential) -> Result<String, AuthError> {
    serde_json::to_string(credential.cred_id()).map_err(|_| AuthError::SecurityKeyConfiguration)
}

pub(crate) fn active_keys(conn: &Connection, user_id: UserId) -> Result<Vec<StoredKey>, AuthError> {
    let rows = conn
        .prepare_cached(
            "SELECT id, credential_json, revision, assertion_revision FROM auth_security_keys \
             WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY created_at, id",
        )?
        .query_map([user_id], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(id, json, revision, assertion_revision)| {
            Ok(StoredKey {
                id,
                user_id,
                credential: serde_json::from_str(&json)
                    .map_err(|_| AuthError::SecurityKeyConfiguration)?,
                revision,
                assertion_revision,
            })
        })
        .collect()
}

pub(crate) fn insert_key(
    conn: &Connection,
    user_id: UserId,
    credential: &KeyCredential,
    label: &str,
    now: UtcTimestamp,
) -> Result<SecurityKeyId, AuthError> {
    let credential_id = credential_id(credential)?;
    let exists: bool = conn
        .prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM auth_security_keys WHERE credential_id = ?1)",
        )?
        .query_row([&credential_id], |row| row.get(0))?;
    if exists {
        return Err(AuthError::SecurityKeyExists);
    }
    let id = SecurityKeyId::generate();
    let json =
        serde_json::to_string(credential).map_err(|_| AuthError::SecurityKeyConfiguration)?;
    conn.prepare_cached(
        "INSERT INTO auth_security_keys (id, user_id, credential_id, credential_json, \
         label, created_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
    )?
    .execute(params![id, user_id, credential_id, json, label, now])?;
    Ok(id)
}

pub(crate) fn update_asserted_key(
    conn: &Connection,
    key: &StoredKey,
    credential: &KeyCredential,
    now: UtcTimestamp,
) -> Result<i64, AuthError> {
    let json =
        serde_json::to_string(credential).map_err(|_| AuthError::SecurityKeyConfiguration)?;
    conn.prepare_cached(
        "UPDATE auth_security_keys SET credential_json = ?4, last_used_at = ?5, \
         assertion_revision = assertion_revision + 1 WHERE id = ?1 AND user_id = ?2 AND revision = ?3 \
         AND assertion_revision = ?6 \
         AND revoked_at IS NULL RETURNING revision",
    )?
    .query_row(params![key.id, key.user_id, key.revision, json, now, key.assertion_revision], |row| row.get(0))
    .optional()?
    .ok_or(AuthError::SecurityKeyUnavailable)
}

pub(crate) fn use_key_method(conn: &Connection, user_id: UserId) -> Result<(), AuthError> {
    let unreachable_hash =
        crate::key_secrets::hash_generated(crate::key_secrets::random_secret()?)?;
    conn.prepare_cached(
        "UPDATE auth_users SET authentication_method = 'security_key', password_hash = ?2, \
         must_change_password = 0, totp_secret = NULL, totp_last_step = NULL WHERE id = ?1",
    )?
    .execute(params![user_id, unreachable_hash])?;
    Ok(())
}

pub(crate) fn revoke_all(
    conn: &Connection,
    user_id: UserId,
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    conn.prepare_cached(
        "UPDATE auth_security_keys SET revoked_at = ?2, revision = revision + 1 \
         WHERE user_id = ?1 AND revoked_at IS NULL",
    )?
    .execute(params![user_id, now])?;
    Ok(())
}
