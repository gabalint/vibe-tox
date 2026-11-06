//! Backup targets, backup runs and update approvals as audit records: the label the audit
//! trail shows, and finding them by name, laboratory day or version (REQ-OPS-001). Labels
//! are resolved at read time and never copied into audit storage.

use lis_audit::{RecordRef, RecordSearch};
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use crate::RuntimeError;

/// Current name of a backup target, or version of an update approval.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, RuntimeError> {
    let sql = match record_type {
        "backup_target" => "SELECT label FROM ops_targets WHERE id = ?1",
        "update_approval" => {
            "SELECT version || CASE WHEN withdrawn_at IS NULL THEN '' ELSE ' (withdrawn)' END \
             FROM ops_update_approvals WHERE id = ?1"
        }
        _ => return Ok(None),
    };
    Ok(conn
        .prepare_cached(sql)?
        .query_row([record_id], |row| row.get(0))
        .optional()?)
}

/// Backup targets by the start of their name, update approvals by the start of their
/// version, and backup runs requested in `day`: the laboratory day `[start, end)` that
/// the searched text names, when it is a date.
pub fn audit_record_search(
    conn: &Connection,
    search: &RecordSearch<'_>,
    day: Option<(UtcTimestamp, UtcTimestamp)>,
) -> Result<Vec<RecordRef>, RuntimeError> {
    let mut records = search.records(
        conn,
        "backup_target",
        "SELECT id FROM ops_targets WHERE label LIKE ?1 ESCAPE '\\' ORDER BY label LIMIT ?2",
    )?;
    records.extend(search.records(
        conn,
        "update_approval",
        "SELECT id FROM ops_update_approvals WHERE version LIKE ?1 ESCAPE '\\' \
         ORDER BY approved_at DESC LIMIT ?2",
    )?);
    if let Some((start, end)) = day
        && search.covers("backup_run")
    {
        let runs = conn
            .prepare_cached(
                "SELECT id FROM ops_backup_runs WHERE requested_at >= ?1 AND requested_at < ?2 \
                 ORDER BY requested_at DESC LIMIT ?3",
            )?
            .query_map(params![start, end, search.limit], |row| {
                Ok(("backup_run", row.get(0)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        records.extend(runs);
    }
    Ok(records)
}
