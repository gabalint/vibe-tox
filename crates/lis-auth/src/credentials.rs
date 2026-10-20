//! Credential methods and the secret-free inventory (REQ-IDN-004, NFR-19).

use lis_core::{Id, UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AuthError;

/// Marker for an enrolled security-key record.
pub enum SecurityKeyRecord {}
/// The public identifier of an enrolled key; not the authenticator credential ID.
pub type SecurityKeyId = Id<SecurityKeyRecord>;

/// The credential policy of an account; security keys never silently fall back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AuthenticationMethod {
    /// A password and an authenticator-app code at sign-in.
    PasswordTotp,
    /// A registered key, with user verification governed by laboratory policy.
    SecurityKey,
}

impl AuthenticationMethod {
    pub(crate) fn from_stored(value: &str) -> rusqlite::Result<Self> {
        match value {
            "password_totp" => Ok(Self::PasswordTotp),
            "security_key" => Ok(Self::SecurityKey),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// A credential-management row; contains no PIN, public-key encoding or secret.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SecurityKeyCredentialView {
    /// The key record used for revocation.
    pub credential_id: SecurityKeyId,
    /// User-supplied name, such as "Everyday key".
    pub label: String,
    /// When registration completed.
    pub created_at: UtcTimestamp,
    /// Most recent successful assertion, if one has completed.
    pub last_used_at: Option<UtcTimestamp>,
    /// Revoked keys remain visible as historical records.
    pub revoked_at: Option<UtcTimestamp>,
    /// Expected revision for revocation.
    #[ts(type = "number")]
    pub revision: i64,
}

/// The account's selected method; the engine authorizes the account query.
pub fn authentication_method(
    conn: &Connection,
    user_id: UserId,
) -> Result<AuthenticationMethod, AuthError> {
    let stored: String = conn
        .prepare_cached("SELECT authentication_method FROM auth_users WHERE id = ?1")?
        .query_row([user_id], |row| row.get(0))
        .optional()?
        .ok_or(AuthError::UserNotFound)?;
    Ok(AuthenticationMethod::from_stored(&stored)?)
}

/// Lists a person's keys. The engine restricts this query to that person or
/// authorized account administration; it never returns credential material.
pub fn list_security_keys(
    conn: &Connection,
    user_id: UserId,
) -> Result<Vec<SecurityKeyCredentialView>, AuthError> {
    Ok(conn
        .prepare_cached(
            "SELECT id, label, created_at, last_used_at, revoked_at, revision \
             FROM auth_security_keys WHERE user_id = ?1 ORDER BY created_at, id",
        )?
        .query_map(params![user_id], |row| {
            Ok(SecurityKeyCredentialView {
                credential_id: row.get(0)?,
                label: row.get(1)?,
                created_at: row.get(2)?,
                last_used_at: row.get(3)?,
                revoked_at: row.get(4)?,
                revision: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

/// The signed-in person's authentication settings, with no credential secrets.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthenticationStatus {
    /// The selected sign-in policy.
    pub method: AuthenticationMethod,
    /// Currently usable personal keys.
    pub credentials: Vec<SecurityKeyCredentialView>,
    /// A one-use offline recovery code has been issued and not consumed.
    pub recovery_available: bool,
    /// Whether lab policy permits touch-only sign-in for newly enrolled keys.
    pub allow_security_key_touch_only: bool,
}

/// The engine authorizes access to this user's settings.
pub fn get_authentication_status(
    conn: &Connection,
    user_id: UserId,
) -> Result<AuthenticationStatus, AuthError> {
    let recovery_available = conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM auth_key_recovery_codes WHERE user_id = ?1 AND consumed_at IS NULL)",
    )?.query_row([user_id], |row| row.get(0))?;
    Ok(AuthenticationStatus {
        method: authentication_method(conn, user_id)?,
        credentials: list_security_keys(conn, user_id)?
            .into_iter()
            .filter(|key| key.revoked_at.is_none())
            .collect(),
        recovery_available,
        allow_security_key_touch_only: crate::policy::current_security_policy(conn)?
            .allow_security_key_touch_only,
    })
}

/// Public sign-in selection metadata. Unknown, disabled and password-only
/// accounts offer no keys; no credential ID or public-key bytes leave the engine.
pub fn list_key_sign_in_choices(
    conn: &Connection,
    username: &str,
) -> Result<Vec<SecurityKeyCredentialView>, AuthError> {
    let username = crate::account::normalize_username(username);
    let Some(user) = crate::repo::find_by_username(conn, &username)? else {
        return Ok(Vec::new());
    };
    if user.status != crate::repo::UserStatus::Active
        || user.authentication_method != AuthenticationMethod::SecurityKey
    {
        return Ok(Vec::new());
    }
    Ok(get_authentication_status(conn, user.id)?.credentials)
}

/// Revokes a key after fresh confirmation. Another person's credential needs
/// account administration and, for a clinical user, quality authority. The
/// final usable key is protected; disable a departing account instead.
pub fn revoke_security_key(
    ctx: &mut lis_store::CommandContext<'_>,
    key_id: SecurityKeyId,
    expected_revision: i64,
    confirmation: &crate::Reauthentication,
    reason: &str,
) -> Result<crate::Attempt<()>, AuthError> {
    use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
    let (actor, _) = crate::sign_in::session_actor(ctx)?;
    let (user_id, revision): (UserId, i64) = ctx
        .tx()
        .prepare_cached(
            "SELECT user_id, revision FROM auth_security_keys WHERE id = ?1 AND revoked_at IS NULL",
        )?
        .query_row([key_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?
        .ok_or(AuthError::SecurityKeyUnavailable)?;
    if actor != user_id {
        crate::duties::require_credential_reset_authority(ctx, user_id)?;
    }
    if revision != expected_revision {
        return Err(AuthError::StaleRevision);
    }
    let reason = crate::account::parse_reason(reason).map_err(|p| p.invalid("reason"))?;
    if crate::key_repo::active_keys(ctx.tx(), user_id)?.len() < 2 {
        return Err(AuthError::LastSecurityKey);
    }
    if let crate::Attempt::Rejected(rejection) = crate::reauthenticate(
        ctx,
        confirmation,
        crate::ReauthPurpose::SecurityKeyRevocation,
    )? {
        return Ok(crate::Attempt::Rejected(rejection));
    }
    ctx.tx()
        .prepare_cached(
            "UPDATE auth_security_keys SET revoked_at = ?2, revision = revision + 1 WHERE id = ?1",
        )?
        .execute(params![key_id, ctx.now()])?;
    crate::repo::bump_revision(ctx.tx(), user_id, ctx.now())?;
    crate::sessions::end_sessions_of(ctx, user_id)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.security_key_revoked",
            Outcome::Succeeded,
        )
        .record("user", user_id)
        .detail("security_key_id", AuditDetail::id(key_id))
        .reason(reason),
    )?;
    Ok(crate::Attempt::Accepted(()))
}
