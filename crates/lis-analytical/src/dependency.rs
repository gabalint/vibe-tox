//! The one home of the analytical dependency gate (REQ-ANA-004,
//! REQ-ANA-005, REQ-ANA-006, REQ-ANA-007): what a result on a position
//! depends on, which of those dependencies block its review, and whether new
//! patient evidence may attach to a position.

use lis_catalog::{AnalyteId, VersionStatus};
use lis_core::{GateReason, InstrumentId};
use rusqlite::Connection;

use crate::error::{AnalyticalError, gate};
use crate::model::{
    CivilDate, ExecutionState, InstrumentStatus, PositionId, PositionRole, PrepBatchId,
    PrepBatchState, QcAssessmentId, QcOutcome, QcScope, QcStatus, RunId, RunState, is_expired,
};
use crate::repo::qc::ScopeKey;
use crate::repo::runs::PositionRow;
use crate::repo::{conflicts, deviations, lots, qc, readiness, runs};
use crate::views::DependencySnapshot;

mod reader;
pub use reader::DependencyRead;

impl DependencySnapshot {
    /// Every causal assessment and deviation, including conditions later cleared.
    pub fn causal_records(
        conn: &Connection,
        position_id: PositionId,
        analyte_id: AnalyteId,
    ) -> Result<Vec<(&'static str, String)>, AnalyticalError> {
        DependencyRead::new(conn).causal_records(position_id, analyte_id)
    }
}

/// The current assessment of a scope: its ID and outcome.
type Current = (QcAssessmentId, QcOutcome);

/// The preparation batch an execution used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prep {
    completed: bool,
    current: Option<Current>,
    /// The current assessment recorded the batch's present revision.
    revision_assessed: bool,
}

/// What the gate decides on, read inside the caller's transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Facts {
    instrument: Option<InstrumentStatus>,
    method_retired: bool,
    lot_expired: bool,
    execution_failed: bool,
    deviation_open: bool,
    deviation_watermark: i64,
    /// `None` when the execution used no preparation batch.
    prep: Option<Prep>,
    qc: Option<Current>,
    qc_stale: bool,
    qc_conflict: bool,
    run_rejected: bool,
}

/// The QC status of one run × analyte from its current assessment's outcome
/// (`None`: pending) and whether the current observations differ from what
/// it cited (or a changed row awaits a decision). The one rule the review
/// gate and the QC work queue share.
pub(crate) fn qc_status(current: Option<QcOutcome>, changed: bool) -> QcStatus {
    match current {
        None => QcStatus::Pending,
        Some(QcOutcome::Rejected) => QcStatus::Rejected,
        Some(QcOutcome::Accepted) if changed => QcStatus::Stale,
        Some(QcOutcome::Accepted) => QcStatus::Accepted,
    }
}

fn qc_gates(facts: &Facts, gates: &mut Vec<GateReason>) {
    match qc_status(facts.qc.map(|(_, outcome)| outcome), facts.qc_stale) {
        QcStatus::Pending => gates.push(gate("qc_pending", "QC for this analyte is not assessed.")),
        QcStatus::Rejected => {
            gates.push(gate("qc_rejected", "QC for this analyte was rejected."));
        }
        QcStatus::Stale => gates.push(gate(
            "qc_stale",
            "The QC data changed after the QC assessment.",
        )),
        QcStatus::Accepted => {}
    }
    if facts.run_rejected && !matches!(facts.qc, Some((_, QcOutcome::Rejected))) {
        gates.push(gate("qc_rejected", "QC for the run was rejected."));
    }
    if facts.qc_conflict {
        gates.push(gate(
            "qc_conflict",
            "A changed QC row for this analyte awaits a decision.",
        ));
    }
}

fn prep_gates(prep: Prep, gates: &mut Vec<GateReason>) {
    if !prep.completed {
        gates.push(gate(
            "prep_not_completed",
            "The preparation batch is not completed.",
        ));
    }
    match prep.current {
        None => gates.push(gate(
            "prep_pending",
            "The preparation batch is not assessed.",
        )),
        Some((_, QcOutcome::Rejected)) => {
            gates.push(gate("prep_rejected", "The preparation batch was rejected."));
        }
        Some((_, QcOutcome::Accepted)) if !prep.revision_assessed => gates.push(gate(
            "prep_stale",
            "The preparation batch changed after its assessment.",
        )),
        Some((_, QcOutcome::Accepted)) => {}
    }
}

/// Decides the gate from `facts`: the snapshot to bind a review to, or every
/// blocking reason.
fn decide(facts: &Facts) -> Result<DependencySnapshot, Vec<GateReason>> {
    let mut gates = Vec::new();
    qc_gates(facts, &mut gates);
    if let Some(prep) = facts.prep {
        prep_gates(prep, &mut gates);
    }
    let others = [
        (
            facts.execution_failed,
            "execution_failed",
            "The test execution was marked failed.",
        ),
        (
            facts.deviation_open,
            "deviation_open",
            "An open deviation affects this result.",
        ),
        (
            facts.lot_expired,
            "lot_expired",
            "A lot was used after its expiry date.",
        ),
        (
            facts.instrument != Some(InstrumentStatus::InService),
            "instrument_not_in_service",
            "The instrument is not in service.",
        ),
        (
            facts.method_retired,
            "method_not_current",
            "The method version is no longer approved.",
        ),
    ];
    for (blocked, code, message) in others {
        if blocked {
            gates.push(gate(code, message));
        }
    }
    match (facts.qc, gates.is_empty()) {
        (Some((qc_assessment_id, _)), true) => Ok(DependencySnapshot {
            qc_assessment_id,
            prep_assessment_id: facts.prep.and_then(|prep| prep.current).map(|(id, _)| id),
            deviation_watermark: facts.deviation_watermark,
        }),
        (Some(_) | None, _) => Err(gates),
    }
}

/// Rechecks everything a result for `analyte_id` on `position_id` depends on
/// (brief §2.4). Gate codes: `qc_pending`, `qc_rejected`, `qc_stale`,
/// `qc_conflict`, `prep_not_completed`, `prep_pending`, `prep_rejected`,
/// `prep_stale`, `execution_failed`, `deviation_open`, `lot_expired`,
/// `instrument_not_in_service`, `method_not_current`. Callers run it inside
/// their own transaction and bind the snapshot to their decision; a review
/// is stale once a later call returns a different QC or preparation
/// assessment or a higher `deviation_watermark`.
pub fn dependency_status(
    conn: &Connection,
    position_id: PositionId,
    analyte_id: AnalyteId,
) -> Result<Result<DependencySnapshot, Vec<GateReason>>, AnalyticalError> {
    DependencyRead::new(conn).status(position_id, analyte_id)
}

/// Whether new patient evidence may attach to `position_id` now
/// (REQ-ANA-001, REQ-ANA-006): its run is open and its execution has not
/// failed. Gate codes: `run_closed`, `execution_failed`. A position that is
/// not a patient position is refused (`WrongState`). The importer calls it
/// in its transaction for every patient row, and before applying a
/// quarantined row, and quarantines a refused row.
pub fn patient_intake_gate(
    conn: &Connection,
    position_id: PositionId,
) -> Result<Result<(), Vec<GateReason>>, AnalyticalError> {
    let position =
        runs::position(conn, position_id)?.ok_or(AnalyticalError::NotFound("position"))?;
    if position.role != PositionRole::Patient {
        return Err(AnalyticalError::WrongState("position"));
    }
    let mut gates = Vec::new();
    if position.run_state != RunState::Open {
        gates.push(gate("run_closed", "The run is closed."));
    }
    if position.execution_state == Some(ExecutionState::Failed) {
        gates.push(gate(
            "execution_failed",
            "The test execution was marked failed.",
        ));
    }
    Ok(if gates.is_empty() { Ok(()) } else { Err(gates) })
}

/// Whether the observations an assessment cites differ from the exact
/// current observations of its run × analyte: one was superseded (or
/// remapped to another analyte by a correction), or a new one arrived.
fn evidence_changed(
    conn: &Connection,
    assessment_id: QcAssessmentId,
    run_id: RunId,
    analyte_id: AnalyteId,
) -> Result<bool, AnalyticalError> {
    // Both lists are ordered by observation ID.
    Ok(qc::evidence(conn, assessment_id)?
        != qc::current_run_analyte_observations(conn, run_id, analyte_id)?)
}

/// The preparation batch's state and current assessment.
fn prep(conn: &Connection, batch: PrepBatchId) -> Result<Prep, AnalyticalError> {
    let view =
        lots::prep_batch(conn, batch)?.ok_or(AnalyticalError::NotFound("preparation batch"))?;
    let current = qc::current_assessment(conn, ScopeKey::PrepBatch(batch))?;
    let revision_assessed = match &current {
        Some(current) => {
            qc::assessed_prep_revision(conn, current.assessment_id)? == Some(view.revision)
        }
        None => false,
    };
    Ok(Prep {
        completed: view.state == PrepBatchState::Completed,
        current: current.map(|current| (current.assessment_id, current.outcome)),
        revision_assessed,
    })
}

/// Whether a lot of the run or the position had expired when the position
/// was added (time of use; withdrawal alone does not affect past work).
fn lot_expired(conn: &Connection, position: &PositionRow) -> Result<bool, AnalyticalError> {
    let mut lot_ids = runs::run_lots(conn, position.run_id)?;
    lot_ids.extend(position.lot_id);
    for lot_id in lot_ids {
        let lot = lots::lot(conn, lot_id)?.ok_or(AnalyticalError::NotFound("lot"))?;
        if is_expired(
            CivilDate::parse("expiresOn", &lot.expires_on)?,
            position.created_at,
        ) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether the instrument's latest status is in service; no status means not
/// ready (R14).
pub(crate) fn instrument_in_service(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<bool, AnalyticalError> {
    Ok(matches!(
        readiness::latest_status(conn, instrument_id)?,
        Some((InstrumentStatus::InService, _))
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear() -> Facts {
        Facts {
            instrument: Some(InstrumentStatus::InService),
            method_retired: false,
            lot_expired: false,
            execution_failed: false,
            deviation_open: false,
            deviation_watermark: 7,
            prep: None,
            qc: Some((QcAssessmentId::generate(), QcOutcome::Accepted)),
            qc_stale: false,
            qc_conflict: false,
            run_rejected: false,
        }
    }

    fn prep(current: Option<Current>) -> Option<Prep> {
        Some(Prep {
            completed: true,
            current,
            revision_assessed: true,
        })
    }

    fn codes(facts: &Facts) -> Vec<String> {
        decide(facts)
            .unwrap_err()
            .into_iter()
            .map(|gate| gate.code)
            .collect()
    }

    #[test]
    fn a_clear_position_yields_its_snapshot() {
        let facts = clear();
        let snapshot = decide(&facts).unwrap();
        assert_eq!(Some(snapshot.qc_assessment_id), facts.qc.map(|qc| qc.0));
        assert_eq!(snapshot.prep_assessment_id, None);
        assert_eq!(snapshot.deviation_watermark, 7);
        let id = QcAssessmentId::generate();
        let with_prep = Facts {
            prep: prep(Some((id, QcOutcome::Accepted))),
            ..clear()
        };
        assert_eq!(decide(&with_prep).unwrap().prep_assessment_id, Some(id));
    }

    #[test]
    fn each_dependency_has_its_gate_code() {
        fn rejected() -> Option<Current> {
            Some((QcAssessmentId::generate(), QcOutcome::Rejected))
        }
        fn accepted() -> Option<Current> {
            Some((QcAssessmentId::generate(), QcOutcome::Accepted))
        }
        type Change = fn(&mut Facts);
        let cases: [(Change, &str); 15] = [
            (|f| f.qc = None, "qc_pending"),
            (|f| f.qc = rejected(), "qc_rejected"),
            (|f| f.run_rejected = true, "qc_rejected"),
            (|f| f.qc_stale = true, "qc_stale"),
            (|f| f.qc_conflict = true, "qc_conflict"),
            (|f| f.prep = prep(rejected()), "prep_rejected"),
            (|f| f.prep = prep(None), "prep_pending"),
            (
                |f| {
                    f.prep = prep(accepted()).map(|p| Prep {
                        completed: false,
                        ..p
                    });
                },
                "prep_not_completed",
            ),
            (
                |f| {
                    f.prep = prep(accepted()).map(|p| Prep {
                        revision_assessed: false,
                        ..p
                    });
                },
                "prep_stale",
            ),
            (|f| f.execution_failed = true, "execution_failed"),
            (|f| f.deviation_open = true, "deviation_open"),
            (|f| f.lot_expired = true, "lot_expired"),
            (|f| f.instrument = None, "instrument_not_in_service"),
            (
                |f| f.instrument = Some(InstrumentStatus::OutOfService),
                "instrument_not_in_service",
            ),
            (|f| f.method_retired = true, "method_not_current"),
        ];
        for (change, code) in cases {
            let mut facts = clear();
            change(&mut facts);
            assert_eq!(codes(&facts), vec![code.to_owned()], "{facts:?}");
        }
    }

    #[test]
    fn a_rejection_is_reported_once_and_staleness_never_masks_it() {
        let rejected = Some((QcAssessmentId::generate(), QcOutcome::Rejected));
        let facts = Facts {
            qc: rejected,
            qc_stale: true,
            run_rejected: true,
            ..clear()
        };
        assert_eq!(codes(&facts), vec!["qc_rejected".to_owned()]);
    }

    #[test]
    fn every_blocking_reason_is_listed() {
        let facts = Facts {
            instrument: None,
            method_retired: true,
            lot_expired: true,
            execution_failed: true,
            deviation_open: true,
            deviation_watermark: 0,
            prep: Some(Prep {
                completed: false,
                current: None,
                revision_assessed: false,
            }),
            qc: None,
            qc_stale: false,
            qc_conflict: true,
            run_rejected: false,
        };
        assert_eq!(codes(&facts).len(), 9);
    }
}
