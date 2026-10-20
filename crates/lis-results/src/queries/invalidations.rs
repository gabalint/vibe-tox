//! Append-only causes that invalidate a technical review or a released
//! result line (REQ-RES-005, REQ-RPT-002, NFR-01).
//!
//! Callers supply a clinical-audit sequence watermark. Every returned
//! [`StaleReason`] names the causal record committed after that watermark;
//! recovery and re-review do not erase those records.

use std::collections::BTreeSet;

use lis_audit::{Stream, succeeded_record_events};
use lis_core::GateReason;
use rusqlite::{Connection, OptionalExtension, params};

use super::read_context::ReviewRead;
use crate::{ResultCause, ResultError, ResultId, ResultRevisionId, TechnicalReviewId};

/// A durable stale-review cause. The reference is the append-only record
/// that caused it, so a later cause always produces a different impact key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaleReason {
    /// Stable machine code.
    pub code: &'static str,
    /// ID of the QC assessment, deviation, validity assessment, result
    /// revision, collection correction or rejected review that caused this reason.
    pub ref_id: String,
}

impl StaleReason {
    pub(super) fn gate_reason(&self) -> GateReason {
        GateReason {
            code: self.code.to_owned(),
            message: message(self.code).to_owned(),
        }
    }
}

/// Every invalidation of `revision_id` committed after `after_clinical_seq`.
/// Results owns the traversal of result, review and analytical dependency
/// history; reporting never reads those tables.
pub fn stale_reasons_after(
    conn: &Connection,
    revision_id: ResultRevisionId,
    after_clinical_seq: i64,
) -> Result<Vec<StaleReason>, ResultError> {
    stale_reasons_after_read(&mut ReviewRead::new(conn), revision_id, after_clinical_seq)
}

pub(super) fn stale_reasons_after_read(
    read: &mut ReviewRead<'_>,
    revision_id: ResultRevisionId,
    after_clinical_seq: i64,
) -> Result<Vec<StaleReason>, ResultError> {
    let conn = read.conn;
    let facts = read.facts(revision_id)?;
    let result_id: ResultId = conn
        .prepare_cached("SELECT result_id FROM res_result_revisions WHERE id = ?1")?
        .query_row([revision_id], |row| row.get(0))?;
    let mut reasons = BTreeSet::new();
    append_later_revisions(
        conn,
        revision_id,
        result_id,
        after_clinical_seq,
        &mut reasons,
    )?;
    for (position_id, analyte_id) in read.evidence(revision_id)? {
        for (code, ref_id) in read.analytical.causal_records(position_id, analyte_id)? {
            let (action, record_type) = match code {
                "deviation_open" => ("analytical.open_deviation", "deviation"),
                "qc_rejected" | "qc_superseded" | "prep_rejected" => {
                    ("analytical.assess_qc", "qc_assessment")
                }
                _ => return Err(ResultError::WrongState("analytical invalidation")),
            };
            if event_after(conn, action, record_type, &ref_id, None, after_clinical_seq)? {
                reasons.insert(StaleReason { code, ref_id });
            }
        }
    }
    if read.validity_required(facts.component_id, facts.specimen_id)? {
        append_validity_changes(conn, facts.specimen_id, after_clinical_seq, &mut reasons)?;
    }
    append_collection_changes(read, facts.specimen_id, after_clinical_seq, &mut reasons)?;
    append_rejected_reviews(conn, result_id, after_clinical_seq, &mut reasons)?;
    Ok(reasons.into_iter().collect())
}

fn append_collection_changes(
    read: &mut ReviewRead<'_>,
    specimen_id: lis_specimens::SpecimenId,
    after: i64,
    reasons: &mut BTreeSet<StaleReason>,
) -> Result<(), ResultError> {
    let conn = read.conn;
    for correction in read.collection(specimen_id)? {
        let ref_id = correction.correction_id.to_string();
        if event_after(
            conn,
            "specimen.correct_collection_time",
            "collection_correction",
            &ref_id,
            None,
            after,
        )? {
            reasons.insert(StaleReason {
                code: "collection_time_changed",
                ref_id,
            });
        }
    }
    Ok(())
}

fn append_later_revisions(
    conn: &Connection,
    reported_id: ResultRevisionId,
    result_id: ResultId,
    after: i64,
    reasons: &mut BTreeSet<StaleReason>,
) -> Result<(), ResultError> {
    let reported_no: i64 = conn
        .prepare_cached("SELECT revision_no FROM res_result_revisions WHERE id = ?1")?
        .query_row([reported_id], |row| row.get(0))?;
    let revisions: Vec<(ResultRevisionId, i64, ResultCause)> = conn
        .prepare_cached(
            "SELECT id, revision_no, cause FROM res_result_revisions \
             WHERE result_id = ?1 AND revision_no > ?2 ORDER BY revision_no",
        )?
        .query_map(params![result_id, reported_no], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    for (revision_id, revision_no, cause) in revisions {
        if revision_event_sequence(conn, revision_id, result_id, revision_no, cause)? > after {
            reasons.insert(StaleReason {
                code: "revision_superseded",
                ref_id: revision_id.to_string(),
            });
        }
    }
    Ok(())
}

fn revision_event_sequence(
    conn: &Connection,
    revision_id: ResultRevisionId,
    result_id: ResultId,
    revision_no: i64,
    cause: ResultCause,
) -> Result<i64, ResultError> {
    let event = match cause {
        ResultCause::Initial | ResultCause::Reprocessed => {
            let attempt: String = conn
                .prepare_cached(
                    "SELECT DISTINCT m.import_attempt_id FROM res_revision_evidence e \
                     JOIN res_measurements m ON m.id = e.measurement_id \
                     WHERE e.revision_id = ?1 LIMIT 1",
                )?
                .query_row([revision_id], |row| row.get(0))
                .optional()?
                .ok_or(ResultError::WrongState("result revision evidence"))?;
            exact_event(conn, "result.import", "import_attempt", &attempt, None)?
        }
        ResultCause::Selected => exact_event(
            conn,
            "result.select_evidence",
            "result",
            &result_id.to_string(),
            Some(revision_no + 1),
        )?,
        ResultCause::Disposition => exact_event(
            conn,
            "result.record_disposition",
            "result",
            &result_id.to_string(),
            Some(revision_no + 1),
        )?,
    };
    event.ok_or(ResultError::WrongState("result revision audit"))
}

fn append_validity_changes(
    conn: &Connection,
    specimen_id: lis_specimens::SpecimenId,
    after: i64,
    reasons: &mut BTreeSet<StaleReason>,
) -> Result<(), ResultError> {
    let ids: Vec<String> = conn
        .prepare_cached(
            "SELECT id FROM res_validity_assessments WHERE specimen_id = ?1 ORDER BY id",
        )?
        .query_map([specimen_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for ref_id in ids {
        if event_after(
            conn,
            "result.assess_validity",
            "validity_assessment",
            &ref_id,
            None,
            after,
        )? {
            reasons.insert(StaleReason {
                code: "validity_assessment_changed",
                ref_id,
            });
        }
    }
    Ok(())
}

fn append_rejected_reviews(
    conn: &Connection,
    result_id: ResultId,
    after: i64,
    reasons: &mut BTreeSet<StaleReason>,
) -> Result<(), ResultError> {
    let ids: Vec<TechnicalReviewId> = conn
        .prepare_cached(
            "SELECT DISTINCT review.id FROM res_technical_reviews review \
             JOIN res_review_items item ON item.review_id = review.id \
             JOIN res_result_revisions revision ON revision.id = item.result_revision_id \
             WHERE revision.result_id = ?1 AND review.decision = 'rejected' ORDER BY review.id",
        )?
        .query_map([result_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for id in ids {
        let ref_id = id.to_string();
        if event_after(
            conn,
            "result.submit_review",
            "technical_review",
            &ref_id,
            None,
            after,
        )? {
            reasons.insert(StaleReason {
                code: "review_rejected",
                ref_id,
            });
        }
    }
    Ok(())
}

fn event_after(
    conn: &Connection,
    action: &str,
    record_type: &str,
    record_id: &str,
    new_revision: Option<i64>,
    after: i64,
) -> rusqlite::Result<bool> {
    Ok(
        exact_event(conn, action, record_type, record_id, new_revision)?
            .is_some_and(|seq| seq > after),
    )
}

fn exact_event(
    conn: &Connection,
    action: &str,
    record_type: &str,
    record_id: &str,
    new_revision: Option<i64>,
) -> rusqlite::Result<Option<i64>> {
    let events = succeeded_record_events(conn, Stream::Clinical, action, record_type, record_id)?;
    Ok(events
        .into_iter()
        .find(|event| new_revision.is_none_or(|revision| event.new_revision == Some(revision)))
        .map(|event| event.seq))
}

fn message(code: &str) -> &'static str {
    match code {
        "revision_superseded" => "A reviewed result revision is no longer current.",
        "qc_rejected" => "QC for this result was rejected after review.",
        "qc_superseded" => "A QC assessment changed after review.",
        "prep_rejected" => "The preparation batch was rejected after review.",
        "deviation_open" => "A deviation affecting this result opened after review.",
        "validity_assessment_changed" => "The specimen-validity assessment changed after review.",
        "collection_time_changed" => "The specimen collection time changed after review.",
        "review_rejected" => "A later technical review rejected this result.",
        _ => "A dependency changed after review.",
    }
}
