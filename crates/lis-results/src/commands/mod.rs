//! Human result-review commands. Every safety decision rechecks its facts in
//! the caller's transaction and records one attributable audit event.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_catalog::{AnalyteId, MethodAnalyte, MethodVersionId};
use lis_core::{Capability, Comparator};
use lis_interfaces::ReceiptId;
use lis_orders::{OrderComponentId, SequenceRole};
use lis_specimens::SpecimenId;
use lis_store::{CommandContext, OutboxItem};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use ts_rs::TS;

use crate::classification::Classification;
use crate::delimited::{FileValueReading, read_value};
use crate::error::{ResultError, require_reason, require_user};
use crate::model::{
    MeasurementForm, MeasurementId, QuarantineItemId, QuarantineReason, QuarantineResolution,
    QuarantineState, ResultCause, ResultId, ResultOutcome, ResultRevisionId, ReviewDecision,
    TechnicalReviewId, ValidityAssessmentId,
};
use crate::queries::{
    current_result, dependency_snapshots, get_import_attempt, next_attempt_no, reflex_parent,
    review_gates,
};
use crate::result_value::{ResultValue, ValueRule, WordRefusal};
use crate::value::SourceValue;
use crate::views::{ImportAttemptDetail, ResultRevisionView, ReviewView, ValidityAssessmentView};

/// Topic used by a reviewer-requested retry of a receipt.
pub const RETRY_IMPORT_TOPIC: &str = "res.import.retry";

/// Selects one or more equivalent measurements as a result's current evidence.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SelectResultEvidenceInput {
    pub result_id: ResultId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub measurement_ids: Vec<MeasurementId>,
    pub reason: String,
}

/// Records an explicit non-source result disposition.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordResultDispositionInput {
    /// Existing result line. Omit only when recording `missing` for a pending analyte.
    pub result_id: Option<ResultId>,
    /// Required with `result_id`; use zero for a pending analyte.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Pending ordered component; supplied only when `result_id` is absent.
    pub component_id: Option<lis_orders::OrderComponentId>,
    /// Pending ordered analyte; supplied only when `result_id` is absent.
    pub analyte_id: Option<lis_catalog::AnalyteId>,
    /// Pending specimen; supplied only when `result_id` is absent.
    pub specimen_id: Option<SpecimenId>,
    pub outcome: ResultOutcome,
    pub reason: String,
}

/// Approves or rejects exact result revisions for one specimen.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SubmitTechnicalReviewInput {
    pub specimen_id: SpecimenId,
    pub decision: ReviewDecision,
    pub result_revision_ids: Vec<ResultRevisionId>,
    pub reason: Option<String>,
}

/// Records a specimen-validity assessment under an approved policy.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordValidityAssessmentInput {
    pub specimen_id: SpecimenId,
    pub policy_version_id: lis_catalog::ValidityPolicyVersionId,
    pub category_code: String,
    pub evidence_revision_ids: Vec<ResultRevisionId>,
    pub supersedes_id: Option<ValidityAssessmentId>,
    pub reason: Option<String>,
}

/// Resolves one quarantined source row.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolveQuarantineItemInput {
    pub item_id: QuarantineItemId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub resolution: QuarantineResolution,
    pub reason: String,
}

/// Requests another semantic attempt of a durable receipt.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RetryImportInput {
    pub receipt_id: ReceiptId,
    pub reason: String,
}

/// Creates a selected revision without changing or deleting any measurement.
mod measured;
mod quarantine;
mod reflex;
mod review;
mod revision_support;
mod revisions;

pub use measured::{UseMeasuredResultInput, use_measured_result};
pub use quarantine::{resolve_quarantine_item, retry_import};
pub use reflex::{
    RecordReflexDecisionInput, ReflexDecision, ReflexTriggerView, pending_reflex_triggers,
    record_reflex_decision,
};
pub use review::{record_validity_assessment, submit_technical_review};
pub(crate) use revision_support::{
    NewRevisionRecord, insert_result_line, insert_revision_record, point_to,
};
pub use revisions::{record_result_disposition, select_result_evidence};

fn clinical(action: &'static str) -> AuditEvent {
    AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded)
}
