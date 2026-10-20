//! SQL for reports, released revisions and withdrawals.

use lis_core::{UserId, UtcTimestamp};
use lis_specimens::SpecimenId;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    CandidateId, ReportCompleteness, ReportId, ReportRelationship, ReportRevisionId,
    RevisionAvailability,
};

/// A report row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReportRow {
    pub(crate) id: ReportId,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) report_number: String,
}

pub(crate) fn report_for_specimen(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<Option<ReportRow>> {
    conn.prepare_cached(
        "SELECT id, specimen_id, report_number FROM rpt_reports WHERE specimen_id = ?1",
    )?
    .query_row([specimen_id], report_row)
    .optional()
}

pub(crate) fn report(conn: &Connection, id: ReportId) -> rusqlite::Result<Option<ReportRow>> {
    conn.prepare_cached("SELECT id, specimen_id, report_number FROM rpt_reports WHERE id = ?1")?
        .query_row([id], report_row)
        .optional()
}

fn report_row(row: &Row<'_>) -> rusqlite::Result<ReportRow> {
    Ok(ReportRow {
        id: row.get(0)?,
        specimen_id: row.get(1)?,
        report_number: row.get(2)?,
    })
}

pub(crate) fn insert_report(
    conn: &Connection,
    report: &ReportRow,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_reports (id, specimen_id, report_number, created_by, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        report.id,
        report.specimen_id,
        report.report_number,
        by,
        at
    ])?;
    Ok(())
}

/// A released revision with its computed availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RevisionRow {
    pub(crate) id: ReportRevisionId,
    pub(crate) report_id: ReportId,
    pub(crate) revision_no: i64,
    pub(crate) candidate_id: CandidateId,
    pub(crate) relationship: ReportRelationship,
    pub(crate) completeness: ReportCompleteness,
    pub(crate) supersedes_revision_id: Option<ReportRevisionId>,
    pub(crate) amendment_reason: Option<String>,
    pub(crate) content_sha256: String,
    pub(crate) artifact_sha256: String,
    pub(crate) released_by: UserId,
    pub(crate) released_at: UtcTimestamp,
    pub(crate) availability: RevisionAvailability,
}

const REVISION_COLUMNS: &str = "r.id, r.report_id, r.revision_no, r.candidate_id, \
     r.relationship, r.completeness, r.supersedes_revision_id, r.amendment_reason, \
     r.content_sha256, r.artifact_sha256, r.released_by, r.released_at, v.availability \
     FROM rpt_revisions r JOIN rpt_v_revisions v ON v.revision_id = r.id";

fn revision_row(row: &Row<'_>) -> rusqlite::Result<RevisionRow> {
    Ok(RevisionRow {
        id: row.get(0)?,
        report_id: row.get(1)?,
        revision_no: row.get(2)?,
        candidate_id: row.get(3)?,
        relationship: row.get(4)?,
        completeness: row.get(5)?,
        supersedes_revision_id: row.get(6)?,
        amendment_reason: row.get(7)?,
        content_sha256: row.get(8)?,
        artifact_sha256: row.get(9)?,
        released_by: row.get(10)?,
        released_at: row.get(11)?,
        availability: row.get(12)?,
    })
}

pub(crate) fn revision(
    conn: &Connection,
    id: ReportRevisionId,
) -> rusqlite::Result<Option<RevisionRow>> {
    conn.prepare_cached(&format!("SELECT {REVISION_COLUMNS} WHERE r.id = ?1"))?
        .query_row([id], revision_row)
        .optional()
}

/// The revision a released candidate became.
pub(crate) fn revision_of_candidate(
    conn: &Connection,
    candidate_id: CandidateId,
) -> rusqlite::Result<Option<RevisionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {REVISION_COLUMNS} WHERE r.candidate_id = ?1"
    ))?
    .query_row([candidate_id], revision_row)
    .optional()
}

/// The latest revision of a report, withdrawn or not.
pub(crate) fn latest_revision(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Option<RevisionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {REVISION_COLUMNS} WHERE r.report_id = ?1 ORDER BY r.revision_no DESC LIMIT 1"
    ))?
    .query_row([report_id], revision_row)
    .optional()
}

/// Every revision of a report, oldest first.
pub(crate) fn revisions(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Vec<RevisionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {REVISION_COLUMNS} WHERE r.report_id = ?1 ORDER BY r.revision_no"
    ))?
    .query_map([report_id], revision_row)?
    .collect()
}

/// Every current revision, oldest release first.
pub(crate) fn current_revisions(conn: &Connection) -> rusqlite::Result<Vec<RevisionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {REVISION_COLUMNS} WHERE v.availability = 'current' ORDER BY r.released_at, r.id"
    ))?
    .query_map([], revision_row)?
    .collect()
}

pub(crate) fn insert_revision(conn: &Connection, r: &RevisionRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_revisions (id, report_id, revision_no, candidate_id, relationship, \
         completeness, supersedes_revision_id, amendment_reason, content_sha256, artifact_sha256, \
         released_by, released_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?
    .execute(params![
        r.id,
        r.report_id,
        r.revision_no,
        r.candidate_id,
        r.relationship,
        r.completeness,
        r.supersedes_revision_id,
        r.amendment_reason,
        r.content_sha256,
        r.artifact_sha256,
        r.released_by,
        r.released_at
    ])?;
    Ok(())
}

/// A report's withdrawal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WithdrawalRow {
    pub(crate) report_id: ReportId,
    pub(crate) revision_id: ReportRevisionId,
    pub(crate) reason: String,
    pub(crate) notice_artifact_sha256: String,
    pub(crate) notice_template_version: i64,
    pub(crate) withdrawn_by: UserId,
    pub(crate) withdrawn_at: UtcTimestamp,
}

pub(crate) fn withdrawal(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Option<WithdrawalRow>> {
    conn.prepare_cached(
        "SELECT report_id, revision_id, reason, notice_artifact_sha256, notice_template_version, \
         withdrawn_by, withdrawn_at FROM rpt_withdrawals WHERE report_id = ?1",
    )?
    .query_row([report_id], |row| {
        Ok(WithdrawalRow {
            report_id: row.get(0)?,
            revision_id: row.get(1)?,
            reason: row.get(2)?,
            notice_artifact_sha256: row.get(3)?,
            notice_template_version: row.get(4)?,
            withdrawn_by: row.get(5)?,
            withdrawn_at: row.get(6)?,
        })
    })
    .optional()
}

pub(crate) fn insert_withdrawal(conn: &Connection, w: &WithdrawalRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_withdrawals (report_id, revision_id, reason, notice_artifact_sha256, \
         notice_template_version, withdrawn_by, withdrawn_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        w.report_id,
        w.revision_id,
        w.reason,
        w.notice_artifact_sha256,
        w.notice_template_version,
        w.withdrawn_by,
        w.withdrawn_at
    ])?;
    Ok(())
}

/// A durable reference to a rendered withdrawal notice. It reconciles the
/// artifact if the later withdrawal transaction loses a race or otherwise
/// fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedWithdrawalNoticeRow {
    pub(crate) artifact_sha256: String,
    pub(crate) report_id: ReportId,
    pub(crate) revision_no: i64,
    pub(crate) template_version: i64,
    pub(crate) staged_by: UserId,
    pub(crate) staged_at: UtcTimestamp,
}

pub(crate) fn staged_withdrawal_notice(
    conn: &Connection,
    artifact_sha256: &str,
) -> rusqlite::Result<Option<StagedWithdrawalNoticeRow>> {
    conn.prepare_cached(
        "SELECT artifact_sha256, report_id, revision_no, template_version, staged_by, staged_at \
         FROM rpt_withdrawal_notice_artifacts WHERE artifact_sha256 = ?1",
    )?
    .query_row([artifact_sha256], |row| {
        Ok(StagedWithdrawalNoticeRow {
            artifact_sha256: row.get(0)?,
            report_id: row.get(1)?,
            revision_no: row.get(2)?,
            template_version: row.get(3)?,
            staged_by: row.get(4)?,
            staged_at: row.get(5)?,
        })
    })
    .optional()
}

pub(crate) fn insert_staged_withdrawal_notice(
    conn: &Connection,
    row: &StagedWithdrawalNoticeRow,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT OR IGNORE INTO rpt_withdrawal_notice_artifacts (artifact_sha256, report_id, revision_no, \
         template_version, staged_by, staged_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        row.artifact_sha256,
        row.report_id,
        row.revision_no,
        row.template_version,
        row.staged_by,
        row.staged_at,
    ])?;
    Ok(())
}

/// Earliest committed report occurrence, ordered by causal report revision,
/// never by wall-clock time. Reads line links without materializing old PDFs/JSON.
pub(crate) fn first_reported_dates(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<std::collections::HashMap<lis_results::ResultRevisionId, UtcTimestamp>> {
    let mut statement = conn.prepare_cached("SELECT l.result_revision_id, r.released_at FROM rpt_revisions r JOIN rpt_candidate_lines l ON l.candidate_id = r.candidate_id WHERE r.report_id = ?1 AND l.disposition = 'reported' ORDER BY r.revision_no")?;
    let mut dates = std::collections::HashMap::new();
    for row in statement.query_map([report_id], |row| Ok((row.get(0)?, row.get(1)?)))? {
        let (id, at) = row?;
        dates.entry(id).or_insert(at);
    }
    Ok(dates)
}
