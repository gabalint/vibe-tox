//! Read-side views of accounts. The engine authorizes each call.

use std::collections::{BTreeSet, HashMap};

use lis_core::{Capability, UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use ts_rs::TS;

use crate::error::AuthError;
use crate::repo::{self, UserStatus};
use crate::roles::{RoleCode, capabilities_of};

/// Whether first-run setup still has to create an administrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SetupStatus {
    /// True while no account exists.
    pub needs_administrator: bool,
    /// The first account requested demo preparation and it has not completed yet.
    pub demo_setup_pending: bool,
}

/// The first-run state.
pub fn setup_status(conn: &Connection) -> Result<SetupStatus, AuthError> {
    Ok(SetupStatus {
        needs_administrator: !repo::any_user(conn)?,
        demo_setup_pending: repo::pending_demo_setup_user(conn)?.is_some(),
    })
}

/// An account as listed in user administration. Holds no credential material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserSummary {
    /// The account.
    pub user_id: UserId,
    /// Lowercase sign-in name.
    pub username: String,
    /// The person's name.
    pub display_name: String,
    /// Whether the account may sign in.
    pub status: UserStatus,
    /// Current roles, in catalog order.
    pub roles: Vec<RoleCode>,
    /// The currently selected sign-in method.
    pub authentication_method: crate::credentials::AuthenticationMethod,
    /// True once the selected second factor is enrolled.
    pub mfa_enrolled: bool,
    /// True while the username is locked after failed attempts.
    pub locked_out: bool,
    /// For optimistic concurrency of administrative changes.
    #[ts(type = "number")]
    pub revision: i64,
}

const SUMMARY_SQL: &str = "SELECT u.id, u.username, u.display_name, u.status, \
     (u.totp_secret IS NOT NULL OR EXISTS (SELECT 1 FROM auth_security_keys k \
      WHERE k.user_id = u.id AND k.revoked_at IS NULL)), \
     EXISTS (SELECT 1 FROM auth_sign_in_failures f WHERE f.username = u.username \
             AND f.locked_until > ?1), \
     u.revision, u.authentication_method FROM auth_users u";

fn summary(row: &Row<'_>) -> rusqlite::Result<UserSummary> {
    Ok(UserSummary {
        user_id: row.get(0)?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        status: row.get(3)?,
        roles: Vec::new(),
        mfa_enrolled: row.get(4)?,
        locked_out: row.get(5)?,
        revision: row.get(6)?,
        authentication_method: crate::credentials::AuthenticationMethod::from_stored(
            &row.get::<_, String>(7)?,
        )?,
    })
}

/// Every account, ordered by username, as of `now` (for the lockout flag).
pub fn list_users(conn: &Connection, now: UtcTimestamp) -> Result<Vec<UserSummary>, AuthError> {
    let mut users = conn
        .prepare_cached(&format!("{SUMMARY_SQL} ORDER BY u.username"))?
        .query_map([now], summary)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut roles: HashMap<UserId, Vec<RoleCode>> = HashMap::new();
    let mut statement =
        conn.prepare_cached("SELECT user_id, role FROM auth_role_grants WHERE revoked_at IS NULL")?;
    for grant in statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))? {
        let (user_id, role) = grant?;
        roles.entry(user_id).or_default().push(role);
    }
    for user in &mut users {
        user.roles = roles.remove(&user.user_id).unwrap_or_default();
        user.roles.sort_unstable();
    }
    Ok(users)
}

/// One account's summary, as of `now`.
pub(crate) fn user_summary(
    conn: &Connection,
    id: UserId,
    now: UtcTimestamp,
) -> Result<UserSummary, AuthError> {
    let mut user = conn
        .prepare_cached(&format!("{SUMMARY_SQL} WHERE u.id = ?2"))?
        .query_row(params![now, id], summary)
        .optional()?
        .ok_or(AuthError::UserNotFound)?;
    user.roles = repo::current_roles(conn, id)?;
    Ok(user)
}

/// The signed-in person, for the session and the engine's authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    /// The account.
    pub user_id: UserId,
    /// Lowercase sign-in name.
    pub username: String,
    /// The person's name.
    pub display_name: String,
    /// Whether the account may still act.
    pub status: UserStatus,
    /// Current roles.
    pub roles: Vec<RoleCode>,
}

impl SessionUser {
    /// What the person may do now: their roles' capabilities while the
    /// account is active, nothing once it is disabled (NFR-08: current authority).
    #[must_use]
    pub fn capabilities(&self) -> BTreeSet<Capability> {
        match self.status {
            UserStatus::Active => capabilities_of(&self.roles),
            UserStatus::Disabled => BTreeSet::new(),
        }
    }
}

/// The current state of account `id`.
pub fn session_user(conn: &Connection, id: UserId) -> Result<SessionUser, AuthError> {
    let (username, display_name, status) = conn
        .prepare_cached("SELECT username, display_name, status FROM auth_users WHERE id = ?1")?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .optional()?
        .ok_or(AuthError::UserNotFound)?;
    Ok(SessionUser {
        user_id: id,
        username,
        display_name,
        status,
        roles: repo::current_roles(conn, id)?,
    })
}

/// Display names of the accounts among `ids` that exist, for the audit viewer.
pub fn display_names(
    conn: &Connection,
    ids: &BTreeSet<UserId>,
) -> Result<HashMap<UserId, String>, AuthError> {
    let mut statement = conn.prepare_cached("SELECT display_name FROM auth_users WHERE id = ?1")?;
    let mut names = HashMap::new();
    for &id in ids {
        if let Some(name) = statement.query_row([id], |row| row.get(0)).optional()? {
            names.insert(id, name);
        }
    }
    Ok(names)
}
