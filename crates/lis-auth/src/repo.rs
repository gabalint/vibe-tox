//! SQL for `auth_users` and `auth_role_grants`.

use lis_core::{UserId, UtcTimestamp};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use ts_rs::TS;

use crate::account::Username;
use crate::credentials::AuthenticationMethod;
use crate::error::AuthError;
use crate::roles::RoleCode;

/// Whether an account may sign in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum UserStatus {
    /// May sign in.
    Active,
    /// May not sign in; the account and its history are kept.
    Disabled,
}

impl ToSql for UserStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }))
    }
}

impl FromSql for UserStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// One stored account.
pub(crate) struct UserRow {
    pub(crate) id: UserId,
    pub(crate) username: Username,
    pub(crate) status: UserStatus,
    pub(crate) password_hash: String,
    pub(crate) must_change_password: bool,
    pub(crate) totp_secret: Option<Vec<u8>>,
    pub(crate) totp_last_step: Option<i64>,
    pub(crate) revision: i64,
    pub(crate) authentication_method: AuthenticationMethod,
}

impl UserRow {
    /// Fails with a conflict unless `expected` is the stored revision.
    pub(crate) fn ensure_revision(&self, expected: i64) -> Result<(), AuthError> {
        if self.revision == expected {
            Ok(())
        } else {
            Err(AuthError::StaleRevision)
        }
    }
}

const USER_COLUMNS: &str = "id, username, status, password_hash, must_change_password, \
                            totp_secret, totp_last_step, revision, authentication_method";

fn user_row(row: &Row<'_>) -> rusqlite::Result<UserRow> {
    Ok(UserRow {
        id: row.get(0)?,
        username: Username::from_stored(row.get(1)?),
        status: row.get(2)?,
        password_hash: row.get(3)?,
        must_change_password: row.get(4)?,
        totp_secret: row.get(5)?,
        totp_last_step: row.get(6)?,
        revision: row.get(7)?,
        authentication_method: AuthenticationMethod::from_stored(&row.get::<_, String>(8)?)?,
    })
}

/// The account with this (normalized) username, if any.
pub(crate) fn find_by_username(
    conn: &Connection,
    username: &str,
) -> Result<Option<UserRow>, AuthError> {
    let sql = format!("SELECT {USER_COLUMNS} FROM auth_users WHERE username = ?1");
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([username], user_row)
        .optional()?)
}

/// The account `id`.
pub(crate) fn load(conn: &Connection, id: UserId) -> Result<UserRow, AuthError> {
    let sql = format!("SELECT {USER_COLUMNS} FROM auth_users WHERE id = ?1");
    conn.prepare_cached(&sql)?
        .query_row([id], user_row)
        .optional()?
        .ok_or(AuthError::UserNotFound)
}

/// True once any account exists.
pub(crate) fn any_user(conn: &Connection) -> Result<bool, AuthError> {
    Ok(conn
        .prepare_cached("SELECT EXISTS (SELECT 1 FROM auth_users)")?
        .query_row([], |row| row.get(0))?)
}

/// Every sealed TOTP secret with the account it is bound to.
pub(crate) fn sealed_secrets(conn: &Connection) -> Result<Vec<(UserId, Vec<u8>)>, AuthError> {
    Ok(conn
        .prepare_cached("SELECT id, totp_secret FROM auth_users WHERE totp_secret IS NOT NULL")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}

/// A new account at revision 1.
pub(crate) struct NewUser<'a> {
    pub(crate) id: UserId,
    pub(crate) username: &'a Username,
    pub(crate) display_name: &'a str,
    pub(crate) password_hash: &'a str,
    pub(crate) must_change_password: bool,
}

/// Inserts `user`, or fails with [`AuthError::UsernameTaken`].
pub(crate) fn insert(
    conn: &Connection,
    user: &NewUser<'_>,
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    if find_by_username(conn, user.username.as_str())?.is_some() {
        return Err(AuthError::UsernameTaken);
    }
    conn.prepare_cached(
        "INSERT INTO auth_users (id, username, display_name, status, password_hash, \
         must_change_password, created_at, updated_at, revision) \
         VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?6, 1)",
    )?
    .execute(params![
        user.id,
        user.username.as_str(),
        user.display_name,
        user.password_hash,
        user.must_change_password,
        now
    ])?;
    Ok(())
}

/// Increments the revision of account `id` after a change an administrator can see.
pub(crate) fn bump_revision(
    conn: &Connection,
    id: UserId,
    now: UtcTimestamp,
) -> Result<i64, AuthError> {
    Ok(conn
        .prepare_cached(
            "UPDATE auth_users SET revision = revision + 1, updated_at = ?2 WHERE id = ?1 \
             RETURNING revision",
        )?
        .query_row(params![id, now], |row| row.get(0))?)
}

/// Replaces the password hash; `must_change` marks it temporary.
pub(crate) fn set_password(
    conn: &Connection,
    id: UserId,
    password_hash: &str,
    must_change: bool,
) -> Result<(), AuthError> {
    conn.prepare_cached(
        "UPDATE auth_users SET password_hash = ?2, must_change_password = ?3 WHERE id = ?1",
    )?
    .execute(params![id, password_hash, must_change])?;
    Ok(())
}

/// Sets the account status.
pub(crate) fn set_status(
    conn: &Connection,
    id: UserId,
    status: UserStatus,
) -> Result<(), AuthError> {
    conn.prepare_cached("UPDATE auth_users SET status = ?2 WHERE id = ?1")?
        .execute(params![id, status])?;
    Ok(())
}

/// Stores the sealed TOTP secret and its last accepted step; `None` removes the enrolment.
pub(crate) fn set_totp(
    conn: &Connection,
    id: UserId,
    enrolment: Option<(&[u8], i64)>,
) -> Result<(), AuthError> {
    let (secret, step) = enrolment.unzip();
    conn.prepare_cached(
        "UPDATE auth_users SET totp_secret = ?2, totp_last_step = ?3 WHERE id = ?1",
    )?
    .execute(params![id, secret, step])?;
    Ok(())
}

/// Records the last accepted TOTP step (replay guard); not a revision change.
pub(crate) fn set_totp_step(conn: &Connection, id: UserId, step: i64) -> Result<(), AuthError> {
    conn.prepare_cached("UPDATE auth_users SET totp_last_step = ?2 WHERE id = ?1")?
        .execute(params![id, step])?;
    Ok(())
}

/// The roles `id` currently holds, in catalog order.
pub(crate) fn current_roles(conn: &Connection, id: UserId) -> Result<Vec<RoleCode>, AuthError> {
    let mut roles = conn
        .prepare_cached(
            "SELECT role FROM auth_role_grants WHERE user_id = ?1 AND revoked_at IS NULL",
        )?
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<RoleCode>>>()?;
    roles.sort_unstable();
    Ok(roles)
}

/// Adds a grant row for `role`.
pub(crate) fn grant(
    conn: &Connection,
    user_id: UserId,
    role: RoleCode,
    granted_by: Option<UserId>,
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    conn.prepare_cached(
        "INSERT INTO auth_role_grants (id, user_id, role, granted_by, granted_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![GrantId::generate(), user_id, role, granted_by, now])?;
    Ok(())
}

/// Marks the current grant of `role` revoked.
pub(crate) fn revoke(
    conn: &Connection,
    user_id: UserId,
    role: RoleCode,
    revoked_by: Option<UserId>,
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    conn.prepare_cached(
        "UPDATE auth_role_grants SET revoked_by = ?3, revoked_at = ?4 \
         WHERE user_id = ?1 AND role = ?2 AND revoked_at IS NULL",
    )?
    .execute(params![user_id, role, revoked_by, now])?;
    Ok(())
}

/// Active accounts other than `user_id` (a one-person laboratory has none).
pub(crate) fn other_active_accounts(conn: &Connection, user_id: UserId) -> Result<i64, AuthError> {
    Ok(conn
        .prepare_cached("SELECT count(*) FROM auth_users WHERE id <> ?1 AND status = 'active'")?
        .query_row([user_id], |row| row.get(0))?)
}

/// Active accounts, other than `except`, that hold `role`.
pub(crate) fn active_holders(
    conn: &Connection,
    role: RoleCode,
    except: Option<UserId>,
) -> Result<i64, AuthError> {
    Ok(conn
        .prepare_cached(
            "SELECT count(*) FROM auth_users u JOIN auth_role_grants g ON g.user_id = u.id \
             WHERE u.id IS NOT ?1 AND u.status = 'active' AND g.role = ?2 \
             AND g.revoked_at IS NULL",
        )?
        .query_row(params![except, role], |row| row.get(0))?)
}

/// Marker for role grant row identifiers.
enum Grant {}
type GrantId = lis_core::Id<Grant>;

pub(crate) fn request_demo_setup(
    conn: &Connection,
    user: UserId,
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    conn.execute(
        "INSERT INTO auth_demo_setup (singleton, user_id, requested_at) VALUES (1, ?1, ?2)",
        params![user, now],
    )?;
    Ok(())
}

pub(crate) fn pending_demo_setup_user(conn: &Connection) -> Result<Option<UserId>, AuthError> {
    Ok(conn
        .prepare_cached("SELECT user_id FROM auth_demo_setup WHERE completed_at IS NULL")?
        .query_row([], |row| row.get(0))
        .optional()?)
}

pub(crate) fn finish_demo_setup(conn: &Connection, now: UtcTimestamp) -> Result<(), AuthError> {
    conn.execute("UPDATE auth_demo_setup SET completed_at = ?1, revision = revision + 1 WHERE completed_at IS NULL", [now])?;
    Ok(())
}
