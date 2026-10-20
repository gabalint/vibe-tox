//! SQL for exports of issued files (append-only, NFR-07).

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, params};

use crate::model::{ExportId, ExportPurpose, ObligationId, ReportId, ReportRevisionId};

/// One export of an issued file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportRow {
    pub(crate) id: ExportId,
    pub(crate) revision_id: Option<ReportRevisionId>,
    pub(crate) withdrawal_report_id: Option<ReportId>,
    pub(crate) purpose: ExportPurpose,
    pub(crate) obligation_id: Option<ObligationId>,
    pub(crate) artifact_sha256: String,
    pub(crate) file_name: String,
    pub(crate) exported_by: UserId,
    pub(crate) exported_at: UtcTimestamp,
}

pub(crate) fn insert_export(conn: &Connection, e: &ExportRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_exports (id, revision_id, withdrawal_report_id, purpose, obligation_id, \
         artifact_sha256, file_name, exported_by, exported_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        e.id,
        e.revision_id,
        e.withdrawal_report_id,
        e.purpose,
        e.obligation_id,
        e.artifact_sha256,
        e.file_name,
        e.exported_by,
        e.exported_at
    ])?;
    Ok(())
}

/// Every export of a report's revisions and notice, oldest first.
pub(crate) fn report_exports(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Vec<ExportRow>> {
    conn.prepare_cached(
        "SELECT e.id, e.revision_id, e.withdrawal_report_id, e.purpose, e.obligation_id, \
         e.artifact_sha256, e.file_name, e.exported_by, e.exported_at FROM rpt_exports e \
         LEFT JOIN rpt_revisions r ON r.id = e.revision_id \
         WHERE r.report_id = ?1 OR e.withdrawal_report_id = ?1 ORDER BY e.exported_at, e.id",
    )?
    .query_map([report_id], |row| {
        Ok(ExportRow {
            id: row.get(0)?,
            revision_id: row.get(1)?,
            withdrawal_report_id: row.get(2)?,
            purpose: row.get(3)?,
            obligation_id: row.get(4)?,
            artifact_sha256: row.get(5)?,
            file_name: row.get(6)?,
            exported_by: row.get(7)?,
            exported_at: row.get(8)?,
        })
    })?
    .collect()
}
