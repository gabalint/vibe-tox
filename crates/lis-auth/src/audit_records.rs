//! Sessions and the workstation as audit records, and finding users and their sessions by
//! username or name (REQ-OPS-001). User labels come from [`crate::display_names`].
//! Labels are resolved at read time and never copied into audit storage.

use lis_audit::{RecordRef, RecordSearch};
use rusqlite::{Connection, OptionalExtension};

use crate::error::AuthError;

/// The name of a session's user, or "This computer" for the workstation: one
/// installation has one workstation, and its computer name is not stored.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, AuthError> {
    let sql = match record_type {
        "session" => {
            "SELECT account.display_name FROM auth_sessions session \
             JOIN auth_users account ON account.id = session.user_id WHERE session.id = ?1"
        }
        "workstation" => "SELECT 'This computer' FROM auth_workstations WHERE id = ?1",
        _ => return Ok(None),
    };
    Ok(conn
        .prepare_cached(sql)?
        .query_row([record_id], |row| row.get(0))
        .optional()?)
}

/// Users whose username or name starts with the searched text, and their sessions,
/// newest first.
pub fn audit_record_search(
    conn: &Connection,
    search: &RecordSearch<'_>,
) -> Result<Vec<RecordRef>, AuthError> {
    let mut records = search.records(
        conn,
        "user",
        "SELECT id FROM auth_users \
         WHERE username LIKE ?1 ESCAPE '\\' OR display_name LIKE ?1 ESCAPE '\\' \
         ORDER BY username LIMIT ?2",
    )?;
    records.extend(search.records(
        conn,
        "session",
        "SELECT session.id FROM auth_sessions session \
         JOIN auth_users account ON account.id = session.user_id \
         WHERE account.username LIKE ?1 ESCAPE '\\' \
         OR account.display_name LIKE ?1 ESCAPE '\\' \
         ORDER BY session.started_at DESC LIMIT ?2",
    )?);
    Ok(records)
}
