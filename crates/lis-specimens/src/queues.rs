//! Work-queue reads for the engine's queue screen (REQ-OPS-004) and release
//! reconciliation. Identifiers, codes and times only: no demographics and
//! no user-typed text, so these reads are not audited. Oldest first.

use lis_core::UtcTimestamp;
use rusqlite::Connection;

use crate::error::SpecimenError;
use crate::model::{AccessionId, ContainerId, LabelJobId, SpecimenAcceptance, SpecimenId};
use crate::repo::queues;

/// A specimen that needs a receipt decision: received, or held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecimenAwaitingDecision {
    /// The specimen.
    pub specimen_id: SpecimenId,
    /// Its accession.
    pub accession_id: AccessionId,
    /// The accession number, the item's PHI-free title.
    pub accession_number: String,
    /// Its matrix.
    pub specimen_type_code: String,
    /// `received` or `held`.
    pub acceptance: SpecimenAcceptance,
    /// Receipt time, or the time of the latest decision for a held specimen.
    pub since: UtcTimestamp,
}

/// A container whose latest label job failed and needs a reprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedLabelJob {
    /// The failed job.
    pub job_id: LabelJobId,
    /// The container.
    pub container_id: ContainerId,
    /// Its printed label code.
    pub label_code: String,
    /// Its accession.
    pub accession_id: AccessionId,
    /// The accession number.
    pub accession_number: String,
    /// The PHI-free failure code.
    pub error_code: String,
    /// When the failure was recorded.
    pub failed_at: UtcTimestamp,
}

/// Received or held specimens awaiting a receipt decision, at most `limit`.
pub fn specimens_awaiting_decision(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<SpecimenAwaitingDecision>, SpecimenError> {
    Ok(queues::awaiting_decision(conn, limit)?)
}

/// Containers whose latest label job failed, excluding disposed containers,
/// at most `limit`. A later reprint request takes the container off the list.
pub fn failed_label_jobs(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<FailedLabelJob>, SpecimenError> {
    Ok(queues::failed_label_jobs(conn, limit)?)
}

/// Specimens accepted, with or without conditions, in `[from, to)`, by the
/// time of the accepting decision (release reconciliation, REQ-OPS-004).
pub fn accepted_specimens_between(
    conn: &Connection,
    from: UtcTimestamp,
    to: UtcTimestamp,
) -> Result<Vec<SpecimenId>, SpecimenError> {
    Ok(queues::accepted_between(conn, from, to)?)
}
