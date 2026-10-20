//! SQL for the work-queue reads (REQ-OPS-004).

use lis_core::UtcTimestamp;
use rusqlite::Connection;

use crate::model::SpecimenId;
use crate::queues::{FailedLabelJob, SpecimenAwaitingDecision};

/// Received or held specimens; a held one waits since its latest decision.
pub(crate) fn awaiting_decision(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<SpecimenAwaitingDecision>> {
    conn.prepare_cached(
        "SELECT s.id, a.id, a.accession_number, s.specimen_type_code, s.acceptance, \
         coalesce((SELECT max(d.decided_at) FROM spc_receipt_decisions d \
         WHERE d.specimen_id = s.id), s.received_at) AS since \
         FROM spc_specimens s JOIN spc_collections col ON col.id = s.collection_id \
         JOIN spc_accessions a ON a.id = col.accession_id \
         WHERE s.acceptance IN ('received', 'held') ORDER BY since, s.id LIMIT ?1",
    )?
    .query_map([limit], |row| {
        Ok(SpecimenAwaitingDecision {
            specimen_id: row.get(0)?,
            accession_id: row.get(1)?,
            accession_number: row.get(2)?,
            specimen_type_code: row.get(3)?,
            acceptance: row.get(4)?,
            since: row.get(5)?,
        })
    })?
    .collect()
}

/// Failed jobs that are their container's latest job, on containers that
/// are not disposed.
pub(crate) fn failed_label_jobs(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<FailedLabelJob>> {
    conn.prepare_cached(
        "SELECT j.id, j.container_id, v.label_code, v.accession_id, v.accession_number, \
         o.error_code, o.recorded_at FROM spc_label_jobs j \
         JOIN spc_label_job_outcomes o ON o.job_id = j.id AND o.outcome = 'failed' \
         JOIN spc_v_containers v ON v.container_id = j.container_id \
         WHERE v.state <> 'disposed' AND NOT EXISTS (SELECT 1 FROM spc_label_jobs later \
         WHERE later.container_id = j.container_id \
         AND (later.requested_at, later.id) > (j.requested_at, j.id)) \
         ORDER BY o.recorded_at, j.id LIMIT ?1",
    )?
    .query_map([limit], |row| {
        Ok(FailedLabelJob {
            job_id: row.get(0)?,
            container_id: row.get(1)?,
            label_code: row.get(2)?,
            accession_id: row.get(3)?,
            accession_number: row.get(4)?,
            error_code: row.get(5)?,
            failed_at: row.get(6)?,
        })
    })?
    .collect()
}

/// Specimens whose accepting decision falls in `[from, to)`. Acceptance is
/// final, so each specimen has at most one such decision.
pub(crate) fn accepted_between(
    conn: &Connection,
    from: UtcTimestamp,
    to: UtcTimestamp,
) -> rusqlite::Result<Vec<SpecimenId>> {
    conn.prepare_cached(
        "SELECT specimen_id FROM spc_receipt_decisions \
         WHERE decision IN ('accepted', 'accepted_conditionally') \
         AND decided_at >= ?1 AND decided_at < ?2 ORDER BY decided_at, specimen_id",
    )?
    .query_map([from, to], |row| row.get(0))?
    .collect()
}
