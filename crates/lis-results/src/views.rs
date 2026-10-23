//! Result-review, import and quarantine read models. Source values remain lexical text.

use lis_analytical::{AttemptKind, PositionId, RunId, SourceRegistrationId};
use lis_catalog::{
    AnalyteId, MethodCertainty, MethodVersionId, OutputScale, ValidityPolicyVersionId,
};
use lis_core::{Comparator, DecimalText, GateReason, InstrumentId, UserId, UtcTimestamp};
use lis_identity::SubjectSummary;
use lis_interfaces::{FileOrigin, ReceiptId};
use lis_orders::{OrderComponentId, SequenceRole};
use lis_specimens::{SpecimenAcceptance, SpecimenId};
use serde::Serialize;
use ts_rs::TS;

use crate::model::{
    ImportAttemptId, ImportFailureCode, ImportOutcome, MeasurementId, QuarantineItemId,
    QuarantineReason, QuarantineState, ResultCause, ResultId, ResultOutcome, ResultRevisionId,
    ReviewDecision, TechnicalReviewId, ValidityAssessmentId,
};

mod files;

pub use files::{AddOnComponentView, ImportFileView, MeasuredNotOrderedView, SetAsideRowView};

/// Counts and outcome of one semantic import attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportAttemptSummary {
    pub import_attempt_id: ImportAttemptId,
    pub receipt_id: ReceiptId,
    #[ts(type = "number")]
    pub attempt_no: i64,
    pub instrument_id: InstrumentId,
    pub outcome: ImportOutcome,
    pub failure_code: Option<ImportFailureCode>,
    #[ts(type = "number")]
    pub rows_total: i64,
    #[ts(type = "number")]
    pub rows_new: i64,
    #[ts(type = "number")]
    pub rows_duplicate: i64,
    #[ts(type = "number")]
    pub rows_reprocessed: i64,
    #[ts(type = "number")]
    pub rows_qc: i64,
    #[ts(type = "number")]
    pub rows_quarantined: i64,
    /// File rows whose analyte is on the not-a-result list (result files brief F10).
    #[ts(type = "number")]
    pub rows_not_result: i64,
    /// File rows listed without a quarantine item (F9, F17).
    #[ts(type = "number")]
    pub rows_set_aside: i64,
    /// New measurements kept outside the ordered scope (F17).
    #[ts(type = "number")]
    pub rows_retained: i64,
    pub processed_at: UtcTimestamp,
    /// How a result file arrived: loaded by hand or picked up from the
    /// watched folder (result files brief §7). Absent for an ASTM receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub origin: Option<FileOrigin>,
}

/// One quarantined source row. Its clinical text is returned only by the audited detail query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuarantineItemView {
    pub item_id: QuarantineItemId,
    pub record_path: String,
    pub sample_label: Option<String>,
    pub test_code: Option<String>,
    pub value_text: Option<String>,
    pub units_text: Option<String>,
    pub reason: QuarantineReason,
    pub state: QuarantineState,
    #[ts(type = "number")]
    pub revision: i64,
}

/// An import attempt with its quarantined and set-aside clinical rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportAttemptDetail {
    #[serde(flatten)]
    pub summary: ImportAttemptSummary,
    /// The registration version (for a file source, its template) used.
    pub registration_id: Option<SourceRegistrationId>,
    /// The file read, for a result-file receipt.
    pub file: Option<ImportFileView>,
    pub quarantine: Vec<QuarantineItemView>,
    pub set_aside: Vec<SetAsideRowView>,
}

/// One immutable patient measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeasurementView {
    pub measurement_id: MeasurementId,
    pub run_id: RunId,
    pub position_id: PositionId,
    pub position_label: String,
    pub attempt_kind: AttemptKind,
    #[ts(type = "number")]
    pub processing_revision_seq: i64,
    pub value_text: Option<String>,
    pub units_text: Option<String>,
    pub status_text: Option<String>,
    pub superseded: bool,
}

/// One immutable result-line revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResultRevisionView {
    pub revision_id: ResultRevisionId,
    #[ts(type = "number")]
    pub revision_no: i64,
    pub cause: ResultCause,
    pub outcome: ResultOutcome,
    pub comparator: Option<Comparator>,
    pub magnitude: Option<String>,
    pub units: Option<String>,
    pub code: Option<String>,
    pub classification: String,
    pub classification_reason: Option<String>,
    pub method_version_id: MethodVersionId,
    pub method_certainty: MethodCertainty,
    pub output_scale: OutputScale,
    pub sequence_role: SequenceRole,
    pub reason: Option<String>,
    pub created_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub created_by_display: Option<String>,
    pub created_at: UtcTimestamp,
    pub evidence: Vec<MeasurementView>,
}

pub(crate) fn result_revision_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResultRevisionView> {
    Ok(ResultRevisionView {
        revision_id: row.get(0)?,
        revision_no: row.get(1)?,
        cause: row.get(2)?,
        outcome: row.get(3)?,
        comparator: row.get(4)?,
        magnitude: row.get(5)?,
        units: row.get(6)?,
        code: row.get(7)?,
        classification: row.get(8)?,
        classification_reason: row.get(9)?,
        method_version_id: row.get(10)?,
        method_certainty: row.get(11)?,
        output_scale: row.get(12)?,
        sequence_role: row.get(13)?,
        reason: row.get(14)?,
        created_by: row.get(15)?,
        created_by_display: None,
        created_at: row.get(16)?,
        evidence: Vec::new(),
    })
}

/// Computed technical-review status; never persisted on a result line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum ReviewStatus {
    None,
    Current { review_id: TechnicalReviewId },
    Stale { reasons: Vec<GateReason> },
}

/// Technical-review state with the reviewer attribution required by report
/// release. The UI-facing [`ReviewStatus`] remains stable for M2 callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseReviewStatus {
    None,
    Current {
        review_id: TechnicalReviewId,
        reviewed_by: UserId,
        reviewed_at: UtcTimestamp,
    },
    Stale {
        reasons: Vec<GateReason>,
    },
}

/// The exact current result revision consumed by reporting and billing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRevision {
    /// When this result revision was recorded; not an inferred test completion time.
    pub created_at: UtcTimestamp,
    pub revision_id: ResultRevisionId,
    pub revision_no: i64,
    pub cause: ResultCause,
    pub outcome: ResultOutcome,
    pub comparator: Option<Comparator>,
    pub magnitude: Option<DecimalText>,
    pub units_text: Option<String>,
    pub code_text: Option<String>,
    pub classification: String,
    pub classification_reason: Option<String>,
    pub method_version_id: MethodVersionId,
    pub certainty: MethodCertainty,
    pub output_scale: OutputScale,
    pub sequence_role: SequenceRole,
}

/// One specimen result line projected for an in-transaction release gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseLine {
    pub result_id: ResultId,
    pub order_component_id: OrderComponentId,
    pub analyte_id: AnalyteId,
    pub purpose: lis_catalog::ComponentPurpose,
    pub current: Option<ReleaseRevision>,
    pub review: ReleaseReviewStatus,
    pub open_quarantine: bool,
}

/// The current specimen-validity assessment, if its evidence is still current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentValidity {
    pub assessment_id: ValidityAssessmentId,
    pub policy_version_id: ValidityPolicyVersionId,
    pub category_code: String,
}

/// A performed-not-ordered detection retained for report and billing review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutOfScopeDetection {
    pub item_id: QuarantineItemId,
    pub import_attempt_id: ImportAttemptId,
    pub position_id: PositionId,
    pub analyte_id: AnalyteId,
    pub source_test_code: String,
    pub state: QuarantineState,
    pub detected_at: UtcTimestamp,
}

/// One result line and all retained evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResultLineView {
    pub result_id: ResultId,
    #[ts(type = "number")]
    pub revision: i64,
    pub component_id: OrderComponentId,
    pub analyte_id: AnalyteId,
    pub analyte_code: String,
    pub analyte_name: String,
    pub purpose: lis_catalog::ComponentPurpose,
    pub current: Option<ResultRevisionView>,
    pub history: Vec<ResultRevisionView>,
    pub unselected_evidence: Vec<MeasurementView>,
    pub gates: Vec<GateReason>,
    pub review_status: ReviewStatus,
}

/// One specimen-validity assessment and its evidence bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ValidityAssessmentView {
    pub assessment_id: ValidityAssessmentId,
    pub policy_version_id: ValidityPolicyVersionId,
    pub category_code: String,
    pub supersedes_id: Option<ValidityAssessmentId>,
    pub reason: Option<String>,
    pub assessed_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub assessed_by_display: Option<String>,
    pub assessed_at: UtcTimestamp,
    pub evidence_revision_ids: Vec<ResultRevisionId>,
}

/// Full audited result-review record for one specimen.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpecimenResults {
    pub subject: SubjectSummary,
    pub accession_number: String,
    pub specimen_id: SpecimenId,
    pub specimen_type_code: String,
    pub acceptance: SpecimenAcceptance,
    pub lines: Vec<ResultLineView>,
    pub validity_required: bool,
    pub current_validity: Option<ValidityAssessmentView>,
    pub validity_history: Vec<ValidityAssessmentView>,
    pub pending_analytes: Vec<PendingAnalyteView>,
    /// Measurements of analytes outside the ordered scope, kept as evidence
    /// (result files brief F17).
    pub measured_not_ordered: Vec<MeasuredNotOrderedView>,
    pub allowed_actions: Vec<String>,
    /// Completed decisions remain visible after their pending task disappears.
    pub reflex_history: Vec<ReflexHistoryEntry>,
}

/// Expected analyte without a current result revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PendingAnalyteView {
    pub component_id: OrderComponentId,
    pub analyte_id: AnalyteId,
    pub analyte_code: String,
    pub analyte_name: String,
}

/// A technical review decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewView {
    pub review_id: TechnicalReviewId,
    pub specimen_id: SpecimenId,
    pub decision: ReviewDecision,
    pub result_revision_ids: Vec<ResultRevisionId>,
    pub reason: Option<String>,
    pub reviewed_by: String,
    pub reviewed_at: UtcTimestamp,
}

/// One specimen in the technical-review queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewQueueEntry {
    /// Current patient identity and identifiers from the audited queue read.
    pub subject: SubjectSummary,
    /// Stable page ordering key; not a claim of clinical causal chronology.
    pub since: UtcTimestamp,
    pub specimen_id: SpecimenId,
    pub accession_number: String,
    pub specimen_type_code: String,
    #[ts(type = "number")]
    pub lines: i64,
    #[ts(type = "number")]
    pub pending: i64,
    #[ts(type = "number")]
    pub stale: i64,
    #[ts(type = "number")]
    pub blocked: i64,
}

/// A completed reflex decision with its immutable evidence reference and subsequent work.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReflexHistoryEntry {
    pub assessment: lis_orders::TriggerAssessmentView,
    pub orderable_code: String,
    pub orderable_name: String,
    pub created_item_state: Option<lis_orders::ItemState>,
}
