//! Explicit, candidate-bound disclosure of receipt evidence (RPT-01).
use lis_specimens::{ReceiptDecisionId, ReceiptDiscrepancyCode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{ReportingError, gate, invalid, required};
use crate::snapshot::{ConditionSection, SpecimenSection, sha256_hex};

/// One deliberate reportability choice; current codes/disposition cannot be omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConditionReportDecision {
    pub decision_id: ReceiptDecisionId,
    pub code: ReceiptDiscrepancyCode,
    pub include_in_report: bool,
}

/// Frozen with the candidate; the prepare audit identifies the acting signatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConditionReportReview {
    pub source_sha256: String,
    pub decisions: Vec<ConditionReportDecision>,
    pub reason: String,
}

/// Internal evidence shown to an authorized preparer, never itself a report preview.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConditionReviewSource {
    pub source_sha256: String,
    pub latest_decision_id: Option<ReceiptDecisionId>,
    pub conditions: Vec<ConditionSection>,
}

pub(crate) fn source(specimen: &SpecimenSection) -> Result<ConditionReviewSource, ReportingError> {
    Ok(ConditionReviewSource {
        source_sha256: sha256_hex(&serde_json::to_string(&(
            specimen.latest_decision_id,
            &specimen.condition_history,
        ))?),
        latest_decision_id: specimen.latest_decision_id,
        conditions: specimen.condition_history.clone(),
    })
}

pub(crate) fn apply(
    specimen: &mut SpecimenSection,
    review: Option<ConditionReportReview>,
) -> Result<(), ReportingError> {
    if specimen.condition_history.is_empty() && review.is_none() {
        return Ok(());
    }
    let mut review = review.ok_or_else(|| {
        invalid(
            "conditionReview",
            "required",
            "Review each receipt condition before preparing the report.",
        )
    })?;
    if review.source_sha256 != source(specimen)?.source_sha256 {
        return Err(invalid(
            "conditionReview",
            "source_changed",
            "Receipt conditions changed. Close this dialog, refresh the workspace and review them again.",
        ));
    }
    review.reason = required("conditionReview.reason", &review.reason, 1000)?;
    let exact = review.decisions.len() == specimen.condition_history.len()
        && specimen.condition_history.iter().all(|condition| {
            review
                .decisions
                .iter()
                .filter(|decision| {
                    decision.decision_id == condition.decision_id && decision.code == condition.code
                })
                .count()
                == 1
        });
    if !exact {
        return Err(invalid(
            "conditionReview",
            "incomplete_choices",
            "Choose whether to include every receipt condition exactly once.",
        ));
    }
    // Preserve canonical source order regardless of the order submitted by a client.
    review.decisions.sort_by_key(|decision| {
        specimen.condition_history.iter().position(|condition| {
            condition.decision_id == decision.decision_id && condition.code == decision.code
        })
    });
    specimen.condition_report_review = Some(review);
    Ok(())
}

/// An older unreviewed candidate may be inspected, but must be prepared again before release.
pub(crate) fn release_gate(specimen: &SpecimenSection) -> Option<lis_core::GateReason> {
    (!specimen.condition_history.is_empty() && specimen.condition_report_review.is_none()).then(||
        gate("condition_review_required", "Prepare a new candidate and explicitly review which receipt conditions belong in the patient report."))
}

/// Safe workspace preview: no unreviewed description or historical condition is disclosed.
pub(crate) fn preview(specimen: &mut SpecimenSection) {
    specimen.condition_report_review = Some(ConditionReportReview {
        source_sha256: String::new(),
        decisions: Vec::new(),
        reason: String::new(),
    });
}

/// Project only patient-report content. Legacy frozen snapshots retain their original mapping.
pub(crate) fn projected(specimen: &SpecimenSection) -> Vec<ConditionSection> {
    let Some(review) = &specimen.condition_report_review else {
        return specimen.condition_history.clone();
    };
    specimen
        .condition_history
        .iter()
        .filter_map(|condition| {
            let include = review.decisions.iter().any(|decision| {
                decision.decision_id == condition.decision_id
                    && decision.code == condition.code
                    && decision.include_in_report
            });
            let current = Some(condition.decision_id) == specimen.latest_decision_id;
            if !include && !current {
                return None;
            }
            let mut reported = condition.clone();
            if !include {
                reported.description = None;
            }
            Some(reported)
        })
        .collect()
}

#[cfg(test)]
mod tests;
