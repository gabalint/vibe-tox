//! Read models of QC: observations, assessments, deviations and the
//! dependency snapshot a review binds to.

use lis_catalog::AnalyteId;
use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

use crate::model::{
    DeviationId, DeviationScope, DeviationState, LotId, PositionId, PrepBatchId, QcAssessmentId,
    QcConflictId, QcConflictState, QcObservationId, QcOutcome, QcScope, QcStatus, RunId,
};

/// QC of one run: observations and assessments per analyte, the run and
/// preparation-batch assessments, and the deviations affecting the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunQcView {
    /// The run.
    pub run_id: RunId,
    /// One entry per analyte of the run's method version.
    pub analytes: Vec<RunQcAnalyteView>,
    /// Observations whose test code maps to no analyte.
    pub unmapped_observations: Vec<QcObservationView>,
    /// Changed QC rows of the run, open ones first, then by arrival.
    pub conflicts: Vec<QcConflictView>,
    /// Whole-run assessments.
    pub run_assessments: QcAssessmentHistory,
    /// Assessments of the preparation batches the run's executions used.
    pub prep_batches: Vec<PrepBatchQcView>,
    /// Deviations on the run, its positions, instrument, lots and batches.
    pub deviations: Vec<DeviationView>,
}

/// QC of one analyte on a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunQcAnalyteView {
    /// The analyte.
    pub analyte_id: AnalyteId,
    /// Its catalog code.
    pub code: String,
    /// Every observation, superseded ones included.
    pub observations: Vec<QcObservationView>,
    /// The run × analyte assessments.
    pub assessments: QcAssessmentHistory,
}

/// The assessments of a preparation batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepBatchQcView {
    /// The batch.
    pub prep_batch_id: PrepBatchId,
    /// Its assessments.
    pub assessments: QcAssessmentHistory,
}

/// The current assessment of one scope and every assessment, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QcAssessmentHistory {
    /// The assessment no other supersedes; none means pending.
    pub current: Option<QcAssessmentView>,
    /// Every assessment of the scope, newest first.
    pub history: Vec<QcAssessmentView>,
}

/// One QC observation as received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QcObservationView {
    /// The observation.
    pub observation_id: QcObservationId,
    /// The position.
    pub position_id: PositionId,
    /// The position's label on the run.
    pub position_label: String,
    /// The control or calibrator lot.
    pub lot_id: Option<LotId>,
    /// The source test code.
    pub source_test_code: String,
    /// The mapped analyte.
    pub analyte_id: Option<AnalyteId>,
    /// The value text, as received.
    pub value_text: Option<String>,
    /// The units text, as received.
    pub units_text: Option<String>,
    /// The flags text, as received.
    pub flags_text: Option<String>,
    /// The result status text, as received.
    pub status_text: Option<String>,
    /// Whether a later re-export superseded it.
    pub superseded: bool,
    /// When it was recorded.
    pub recorded_at: UtcTimestamp,
}

/// A changed QC row the source registration cannot classify (brief §3.5,
/// R5). While open it blocks results depending on its run × analyte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QcConflictView {
    /// The conflict.
    pub conflict_id: QcConflictId,
    /// The position.
    pub position_id: PositionId,
    /// The stored observation the row contradicts.
    pub conflicts_with_id: QcObservationId,
    /// The source test code, as received.
    pub source_test_code: String,
    /// The analyte the registration in force at receipt mapped the code to.
    pub analyte_id: Option<AnalyteId>,
    /// The value text, as received.
    pub value_text: Option<String>,
    /// The units text, as received.
    pub units_text: Option<String>,
    /// The flags text, as received.
    pub flags_text: Option<String>,
    /// The result status text, as received.
    pub status_text: Option<String>,
    /// Open, accepted or rejected.
    pub state: QcConflictState,
    /// The superseding observation an acceptance appended.
    pub observation_id: Option<QcObservationId>,
    /// Why it was resolved, as entered.
    pub reason: Option<String>,
    /// Who resolved it.
    pub resolved_by: Option<String>,
    /// When.
    pub resolved_at: Option<UtcTimestamp>,
    /// When the row arrived.
    pub recorded_at: UtcTimestamp,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// One QC assessment (REQ-ANA-004, REQ-ANA-005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QcAssessmentView {
    /// The assessment.
    pub assessment_id: QcAssessmentId,
    /// What it covers.
    pub scope: QcScope,
    /// Accepted or rejected.
    pub outcome: QcOutcome,
    /// The assessment it superseded.
    pub supersedes_id: Option<QcAssessmentId>,
    /// Why, as entered; required when superseding.
    pub reason: Option<String>,
    /// Who assessed.
    pub assessed_by: String,
    /// When.
    pub assessed_at: UtcTimestamp,
    /// The observations it cites.
    pub evidence_observation_ids: Vec<QcObservationId>,
}

/// A quality deviation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviationView {
    /// The deviation.
    pub deviation_id: DeviationId,
    /// What it applies to.
    pub scope: DeviationScope,
    /// The ID of the instrument, lot, batch, run or position.
    pub scope_id: String,
    /// The one analyte it is limited to, if any.
    pub analyte_id: Option<AnalyteId>,
    /// What happened, as entered.
    pub description: String,
    /// Why it was opened, as entered.
    pub reason: String,
    /// Open or closed.
    pub state: DeviationState,
    /// The disposition, as entered at closing.
    pub disposition: Option<String>,
    /// Why it was closed, as entered.
    pub close_reason: Option<String>,
    /// Who opened it.
    pub opened_by: String,
    /// When.
    pub opened_at: UtcTimestamp,
    /// Who closed it.
    pub closed_by: Option<String>,
    /// When.
    pub closed_at: Option<UtcTimestamp>,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// The assessments a result's review depends on (brief §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencySnapshot {
    /// The current accepted run × analyte assessment.
    pub qc_assessment_id: QcAssessmentId,
    /// The current accepted preparation-batch assessment, when the execution has a batch.
    pub prep_assessment_id: Option<QcAssessmentId>,
    /// The highest sequence number of the deviations applicable to the
    /// result, open or closed; 0 when there are none. A review stores it: a
    /// higher value later means a deviation was opened after the review, and
    /// the review stays stale even once that deviation is closed.
    pub deviation_watermark: i64,
}

/// A run × analyte whose QC needs a decision (REQ-OPS-004 work queue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QcWorkItem {
    /// The run.
    pub run_id: RunId,
    /// The analyte.
    pub analyte_id: AnalyteId,
    /// `pending`, `stale` or `rejected`.
    pub status: QcStatus,
    /// The earliest unassessed observation, or the rejection time.
    pub since: UtcTimestamp,
}
