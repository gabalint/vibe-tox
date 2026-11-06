//! Reports, released revisions and report exports as audit records: the report number the
//! audit trail shows, and finding them by it (REQ-OPS-001). Report numbers are PHI-free
//! (accession number and specimen type code); labels are resolved at read time and never
//! copied into audit storage.

use lis_audit::{RecordRef, RecordSearch};
use rusqlite::{Connection, OptionalExtension};

use crate::error::ReportingError;

/// Current report number of a report or report export audit record, with the revision
/// number when the record is a released revision.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, ReportingError> {
    let sql = match record_type {
        "report" => "SELECT report_number FROM rpt_reports WHERE id = ?1",
        "report_revision" => {
            "SELECT report.report_number || ' revision ' || revision.revision_no \
             FROM rpt_revisions revision \
             JOIN rpt_reports report ON report.id = revision.report_id WHERE revision.id = ?1"
        }
        "report_export" => {
            "SELECT report.report_number FROM rpt_exports export \
             LEFT JOIN rpt_revisions revision ON revision.id = export.revision_id \
             JOIN rpt_reports report \
             ON report.id = coalesce(revision.report_id, export.withdrawal_report_id) \
             WHERE export.id = ?1"
        }
        _ => return Ok(None),
    };
    Ok(conn
        .prepare_cached(sql)?
        .query_row([record_id], |row| row.get(0))
        .optional()?)
}

/// Reports, their released revisions and their exports by the start of the report number,
/// which starts with the accession number.
pub fn audit_record_search(
    conn: &Connection,
    search: &RecordSearch<'_>,
) -> Result<Vec<RecordRef>, ReportingError> {
    let mut records = search.records(
        conn,
        "report",
        "SELECT id FROM rpt_reports WHERE report_number LIKE ?1 ESCAPE '\\' \
         ORDER BY report_number LIMIT ?2",
    )?;
    records.extend(search.records(
        conn,
        "report_revision",
        "SELECT revision.id FROM rpt_revisions revision \
         JOIN rpt_reports report ON report.id = revision.report_id \
         WHERE report.report_number LIKE ?1 ESCAPE '\\' \
         ORDER BY report.report_number, revision.revision_no LIMIT ?2",
    )?);
    records.extend(search.records(
        conn,
        "report_export",
        "SELECT export.id FROM rpt_exports export \
         LEFT JOIN rpt_revisions revision ON revision.id = export.revision_id \
         JOIN rpt_reports report \
         ON report.id = coalesce(revision.report_id, export.withdrawal_report_id) \
         WHERE report.report_number LIKE ?1 ESCAPE '\\' \
         ORDER BY export.exported_at DESC LIMIT ?2",
    )?);
    Ok(records)
}
