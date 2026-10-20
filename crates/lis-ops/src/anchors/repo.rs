//! SQL of `ops_anchor_exports` and `ops_anchor_export_ranges`.

use lis_audit::Stream;
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::AnchorExportId;
use super::lag::{LastExportView, SealRangeView};
use crate::targets::TargetId;

/// One anchor file to record.
pub(super) struct NewExport<'a> {
    pub(super) id: AnchorExportId,
    pub(super) target_id: TargetId,
    pub(super) file_name: &'a str,
    pub(super) exported_at: UtcTimestamp,
    /// The user's ID text for a manual export; `None` for the job runner.
    pub(super) exported_by: Option<&'a str>,
}

/// One stream's seals in an anchor file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SealRange {
    pub(super) stream: Stream,
    pub(super) first_seal_no: i64,
    pub(super) last_seal_no: i64,
}

/// One recorded anchor file, for the export list.
pub(super) struct ExportRow {
    pub(super) id: AnchorExportId,
    pub(super) target_label: String,
    pub(super) file_name: String,
    pub(super) exported_at: UtcTimestamp,
}

/// The newest seal number of `stream` exported to `target_id`.
pub(super) fn last_exported_seal(
    conn: &Connection,
    target_id: TargetId,
    stream: Stream,
) -> rusqlite::Result<Option<i64>> {
    conn.prepare_cached(
        "SELECT max(r.last_seal_no) FROM ops_anchor_export_ranges r \
         JOIN ops_anchor_exports e ON e.id = r.export_id \
         WHERE e.target_id = ?1 AND r.stream = ?2",
    )?
    .query_row(params![target_id, stream.as_str()], |row| row.get(0))
}

pub(super) fn insert_export(
    tx: &Transaction<'_>,
    export: &NewExport<'_>,
    ranges: &[SealRange],
) -> rusqlite::Result<()> {
    let kind = if export.exported_by.is_some() {
        "manual"
    } else {
        "scheduled"
    };
    tx.prepare_cached(
        "INSERT INTO ops_anchor_exports (id, target_id, file_name, exported_at, kind, \
         exported_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        export.id,
        export.target_id,
        export.file_name,
        export.exported_at,
        kind,
        export.exported_by,
    ])?;
    let mut insert_range = tx.prepare_cached(
        "INSERT INTO ops_anchor_export_ranges (export_id, stream, first_seal_no, last_seal_no) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for range in ranges {
        insert_range.execute(params![
            export.id,
            range.stream.as_str(),
            range.first_seal_no,
            range.last_seal_no,
        ])?;
    }
    Ok(())
}

/// The newest recorded anchor file.
pub(super) fn last_export(conn: &Connection) -> rusqlite::Result<Option<LastExportView>> {
    conn.prepare_cached(
        "SELECT e.exported_at, t.label FROM ops_anchor_exports e \
         JOIN ops_targets t ON t.id = e.target_id ORDER BY e.exported_at DESC, e.id DESC LIMIT 1",
    )?
    .query_row([], |row| {
        Ok(LastExportView {
            at: row.get(0)?,
            target_label: row.get(1)?,
        })
    })
    .optional()
}

/// The newest `limit` anchor files.
pub(super) fn list_exports(conn: &Connection, limit: u32) -> rusqlite::Result<Vec<ExportRow>> {
    conn.prepare_cached(
        "SELECT e.id, t.label, e.file_name, e.exported_at FROM ops_anchor_exports e \
         JOIN ops_targets t ON t.id = e.target_id ORDER BY e.exported_at DESC, e.id DESC LIMIT ?1",
    )?
    .query_map([limit], |row| {
        Ok(ExportRow {
            id: row.get(0)?,
            target_label: row.get(1)?,
            file_name: row.get(2)?,
            exported_at: row.get(3)?,
        })
    })?
    .collect()
}

/// The seal ranges of one anchor file, in stream order.
pub(super) fn export_ranges(
    conn: &Connection,
    export_id: AnchorExportId,
) -> rusqlite::Result<Vec<SealRangeView>> {
    conn.prepare_cached(
        "SELECT stream, first_seal_no, last_seal_no FROM ops_anchor_export_ranges \
         WHERE export_id = ?1 ORDER BY CASE stream WHEN 'clinical' THEN 1 \
         WHEN 'security' THEN 2 ELSE 3 END",
    )?
    .query_map([export_id], |row| {
        Ok(SealRangeView {
            stream: row.get(0)?,
            first_seal_no: row.get(1)?,
            last_seal_no: row.get(2)?,
        })
    })?
    .collect()
}
