//! Instruments as audit records: the label the audit trail shows, and finding them by
//! code or name (REQ-OPS-001). Labels are resolved at read time and never copied into
//! audit storage.

use lis_audit::{RecordRef, RecordSearch};
use rusqlite::{Connection, OptionalExtension};

use crate::error::InterfaceError;

/// Current code and name of an instrument audit record.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, InterfaceError> {
    if record_type != "instrument" {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached("SELECT code || ' · ' || name FROM ifc_instruments WHERE id = ?1")?
        .query_row([record_id], |row| row.get(0))
        .optional()?)
}

/// Instruments whose code or name starts with the searched text.
pub fn audit_record_search(
    conn: &Connection,
    search: &RecordSearch<'_>,
) -> Result<Vec<RecordRef>, InterfaceError> {
    Ok(search.records(
        conn,
        "instrument",
        "SELECT id FROM ifc_instruments \
         WHERE code LIKE ?1 ESCAPE '\\' OR name LIKE ?1 ESCAPE '\\' ORDER BY code LIMIT ?2",
    )?)
}
