//! Exact analytical and validity dependencies captured by an approved
//! technical review (REQ-RES-005, NFR-01).

use super::review::{current_validity_id, evidence_positions, revision_facts, validity_required};
use super::*;

/// Dependencies to bind to an approved review after `review_gates` passes.
pub(crate) fn dependency_snapshots(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<Vec<(&'static str, String)>, ResultError> {
    let facts = revision_facts(conn, revision_id)?;
    let mut dependencies = Vec::new();
    let mut deviation_watermark = None;
    for (position_id, analyte_id) in evidence_positions(conn, revision_id)? {
        let snapshot = lis_analytical::dependency_status(conn, position_id, analyte_id)?
            .map_err(ResultError::GateBlocked)?;
        dependencies.push(("qc_assessment", snapshot.qc_assessment_id.to_string()));
        if let Some(prep) = snapshot.prep_assessment_id {
            dependencies.push(("prep_assessment", prep.to_string()));
        }
        deviation_watermark = Some(
            deviation_watermark
                .unwrap_or(0)
                .max(snapshot.deviation_watermark),
        );
    }
    if let Some(watermark) = deviation_watermark {
        dependencies.push(("deviation_watermark", watermark.to_string()));
    }
    if validity_required(conn, facts.component_id, facts.specimen_id)?
        && let Some(validity) = current_validity_id(conn, facts.specimen_id)?
    {
        dependencies.push(("validity_assessment", validity.to_string()));
    }
    dependencies.sort_unstable();
    dependencies.dedup();
    Ok(dependencies)
}
