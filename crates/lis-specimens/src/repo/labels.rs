//! SQL for label jobs and their outcomes.

use lis_core::UtcTimestamp;
use lis_identity::SubjectId;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{ContainerId, LabelJobId, LabelJobKind, PrinterId};
use crate::views::LabelJobView;

pub(crate) struct NewJob<'a> {
    pub(crate) id: LabelJobId,
    pub(crate) container_id: ContainerId,
    pub(crate) printer_id: PrinterId,
    pub(crate) template_id: &'a str,
    pub(crate) template_version: u32,
    pub(crate) kind: LabelJobKind,
    pub(crate) reason: Option<&'a str>,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_job(conn: &Connection, new: &NewJob<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_label_jobs (id, container_id, printer_id, template_id, template_version, \
         kind, reason, requested_by, requested_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        new.id,
        new.container_id,
        new.printer_id,
        new.template_id,
        new.template_version,
        new.kind,
        new.reason,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn container_has_job(
    conn: &Connection,
    container_id: ContainerId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM spc_label_jobs WHERE container_id = ?1)")?
        .query_row([container_id], |row| row.get(0))
}

const JOB_COLUMNS: &str = "j.id, j.container_id, j.printer_id, j.kind, j.requested_at, \
    o.outcome, o.error_code FROM spc_label_jobs j \
    LEFT JOIN spc_label_job_outcomes o ON o.job_id = j.id";

fn job_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<LabelJobView> {
    Ok(LabelJobView {
        job_id: row.get(0)?,
        container_id: row.get(1)?,
        printer_id: row.get(2)?,
        kind: row.get(3)?,
        requested_at: row.get(4)?,
        outcome: row.get(5)?,
        error_code: row.get(6)?,
    })
}

pub(crate) fn container_jobs(
    conn: &Connection,
    container_id: ContainerId,
) -> rusqlite::Result<Vec<LabelJobView>> {
    conn.prepare_cached(&format!(
        "SELECT {JOB_COLUMNS} WHERE j.container_id = ?1 ORDER BY j.requested_at, j.id"
    ))?
    .query_map([container_id], job_view)?
    .collect()
}

pub(crate) fn job(conn: &Connection, id: LabelJobId) -> rusqlite::Result<Option<LabelJobView>> {
    conn.prepare_cached(&format!("SELECT {JOB_COLUMNS} WHERE j.id = ?1"))?
        .query_row([id], job_view)
        .optional()
}

/// What a label shows besides the subject's demographics.
pub(crate) struct JobContent {
    pub(crate) label_code: String,
    pub(crate) accession_number: String,
    pub(crate) specimen_type_code: String,
    pub(crate) subject_id: SubjectId,
    pub(crate) printer_id: PrinterId,
    pub(crate) template_id: String,
    pub(crate) template_version: u32,
    pub(crate) has_outcome: bool,
}

pub(crate) fn job_content(
    conn: &Connection,
    id: LabelJobId,
) -> rusqlite::Result<Option<JobContent>> {
    conn.prepare_cached(
        "SELECT v.label_code, v.accession_number, v.specimen_type_code, v.subject_id, \
         j.printer_id, j.template_id, j.template_version, \
         EXISTS (SELECT 1 FROM spc_label_job_outcomes o WHERE o.job_id = j.id) \
         FROM spc_label_jobs j JOIN spc_v_containers v ON v.container_id = j.container_id \
         WHERE j.id = ?1",
    )?
    .query_row([id], |row| {
        Ok(JobContent {
            label_code: row.get(0)?,
            accession_number: row.get(1)?,
            specimen_type_code: row.get(2)?,
            subject_id: row.get(3)?,
            printer_id: row.get(4)?,
            template_id: row.get(5)?,
            template_version: row.get(6)?,
            has_outcome: row.get(7)?,
        })
    })
    .optional()
}

pub(crate) struct NewOutcome<'a> {
    pub(crate) job_id: LabelJobId,
    pub(crate) outcome: crate::model::LabelOutcome,
    pub(crate) error_code: Option<&'a str>,
    pub(crate) label_sha256: Option<&'a str>,
    pub(crate) truncated: Option<bool>,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_outcome(conn: &Connection, new: &NewOutcome<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_label_job_outcomes (job_id, outcome, error_code, label_sha256, \
         truncated, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        new.job_id,
        new.outcome,
        new.error_code,
        new.label_sha256,
        new.truncated,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn job_has_outcome(conn: &Connection, id: LabelJobId) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM spc_label_job_outcomes WHERE job_id = ?1)")?
        .query_row([id], |row| row.get(0))
}
