//! Original collection evidence and source-backed timing corrections (REQ-SPC-002).

use lis_core::{Id, UtcTimestamp};
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::repo::{accessions, collection_corrections as repo};
use crate::{
    AccessionId, CollectionId, CollectionPrecision, CollectionView, SpecimenError, SpecimenId,
};

/// One immutable correction of collection timing.
pub enum CollectionCorrection {}
/// Identity of a correction and its clinical timing review.
pub type CollectionCorrectionId = Id<CollectionCorrection>;

/// A correction with its separately recorded quality review.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionCorrectionView {
    pub correction_id: CollectionCorrectionId,
    #[ts(type = "number")]
    pub revision: i64,
    pub precision: CollectionPrecision,
    pub collected_at: Option<UtcTimestamp>,
    pub utc_offset_minutes: Option<i32>,
    pub collected_date: Option<String>,
    pub source_document_ref: String,
    pub reason: String,
    pub recorded_by: String,
    pub recorded_at: UtcTimestamp,
    pub resolved_at: Option<UtcTimestamp>,
    pub resolved_by: Option<String>,
    pub resolution_reason: Option<String>,
    pub allowed_actions: Vec<String>,
}

/// Collection history; original evidence is never overwritten.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionHistory {
    pub accession_id: AccessionId,
    pub accession_number: String,
    pub subject: lis_identity::SubjectSummary,
    pub collection_id: CollectionId,
    #[ts(type = "number")]
    pub revision: i64,
    pub original: CollectionView,
    pub current: CollectionView,
    pub corrections: Vec<CollectionCorrectionView>,
    pub allowed_actions: Vec<String>,
}

/// Sensitive read; the engine authorizes and audits before returning it.
pub fn get_collection_history(
    conn: &Connection,
    accession_id: AccessionId,
) -> Result<CollectionHistory, SpecimenError> {
    let accession =
        accessions::accession(conn, accession_id)?.ok_or(SpecimenError::NotFound("accession"))?;
    let mut current = accessions::collection(conn, accession_id)?;
    let mut original = repo::original(conn, accession_id)?;
    current.observations = accessions::observations(conn, current.collection_id)?;
    original.observations = current.observations.clone();
    let corrections = repo::history(conn, current.collection_id)?;
    Ok(CollectionHistory {
        accession_id,
        accession_number: accession.number,
        subject: lis_identity::subject_summary(conn, accession.subject_id)?,
        collection_id: current.collection_id,
        revision: corrections.last().map_or(1, |row| row.revision),
        original,
        current,
        corrections,
        allowed_actions: vec!["correct_collection_time".to_owned()],
    })
}

/// One accession needing quality review of corrected collection timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionReviewQueueItem {
    pub accession_id: AccessionId,
    pub accession_number: String,
    pub recorded_at: UtcTimestamp,
}

/// PHI-free queue references, one row per accession, oldest pending correction first.
/// Internal composition may request a sentinel row or every pending item; the engine
/// validates the operator-facing page bound before composing its stable cursor page.
pub fn pending_collection_reviews(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<CollectionReviewQueueItem>, SpecimenError> {
    repo::pending(conn, limit)
}

/// Retained correction history for report-impact evaluation, including resolved reviews.
pub fn collection_corrections_for_specimen(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<CollectionCorrectionView>, SpecimenError> {
    let accession_id = crate::specimen_accession(conn, specimen_id)?;
    let collection = accessions::collection(conn, accession_id)?;
    repo::history(conn, collection.collection_id)
}

pub(crate) fn append_review_gate(
    conn: &Connection,
    specimen_id: SpecimenId,
    gates: &mut Vec<lis_core::GateReason>,
) -> Result<(), SpecimenError> {
    if repo::review_pending(conn, specimen_id)? {
        gates.push(lis_core::GateReason {
            code: "collection_time_review".to_owned(),
            message: "The corrected collection time needs a quality review.".to_owned(),
        });
    }
    Ok(())
}
