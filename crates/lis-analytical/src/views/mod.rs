//! Read models returned by commands and queries. IDs, decimals and
//! timestamps serialize as strings. No view carries demographics.

use lis_catalog::{AnalyteId, MethodVersionId};
use lis_core::{DecimalText, InstrumentId, UtcTimestamp};
use lis_interfaces::InstrumentTransport;
use lis_orders::OrderComponentId;
use lis_specimens::ContainerId;
use serde::Serialize;
use ts_rs::TS;

mod qc;

pub use qc::{
    DependencySnapshot, DeviationView, PrepBatchQcView, QcAssessmentHistory, QcAssessmentView,
    QcConflictView, QcObservationView, QcWorkItem, RunQcAnalyteView, RunQcView,
};

use crate::file_layout::FileLayout;
use crate::model::{
    AttemptKind, ExecutionId, ExecutionState, InstrumentStatus, LotId, LotState, MaterialKind,
    PositionId, PositionRole, PrepBatchId, PrepBatchState, PrepStepId, RegistrationState,
    RevisionIndicator, RunId, RunState, SampleIdField, SourceRegistrationId,
};

/// An instrument's readiness (REQ-ANA-007). No status event means not ready.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstrumentReadinessView {
    /// The instrument.
    pub instrument_id: InstrumentId,
    /// Its configured name.
    pub name: String,
    /// The latest recorded status, if any.
    pub status: Option<InstrumentStatus>,
    /// When the latest status was recorded.
    pub since: Option<UtcTimestamp>,
    /// How its results reach the LIS: ASTM/AU over TCP or serial, or result files.
    pub transport: InstrumentTransport,
    /// The active source registration, if any.
    pub active_registration: Option<RegistrationSummary>,
}

/// The sender of an active registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegistrationSummary {
    /// The registration.
    pub registration_id: SourceRegistrationId,
    /// Registered ASTM H.5 sender name, or declared AU/file software identity.
    pub sender: String,
    /// The registered sender software version.
    pub version: String,
    /// The source is a file template: for a file source the sender is the
    /// declared software name and version (result files brief F2).
    pub file_template: bool,
}

/// A source registration with its test-code map (REQ-ANA-003), plus an
/// AU profile or file layout when that format needs explicit declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceRegistrationView {
    /// The registration.
    pub registration_id: SourceRegistrationId,
    /// The sending instrument.
    pub instrument_id: InstrumentId,
    /// ASTM H.5 sender name; declared device/software identity for AU and files.
    pub sender_name: String,
    /// ASTM header software version; declared version for AU and files.
    pub sender_version: String,
    /// Which order field carries the sample label.
    pub sample_id_field: SampleIdField,
    /// How the source marks corrected re-exports.
    pub revision_indicator: RevisionIndicator,
    /// Whether the source reports final (dilution-corrected) concentrations.
    pub reports_final_concentration: bool,
    /// Draft, active or retired.
    pub state: RegistrationState,
    /// Who recorded it.
    pub authored_by: String,
    /// When it was recorded.
    pub authored_at: UtcTimestamp,
    /// Who approved it; the author only as the separation-of-duties policy
    /// allowed (ADR 0015).
    pub approved_by: Option<String>,
    /// When it was approved.
    pub approved_at: Option<UtcTimestamp>,
    /// When a newer approval retired it.
    pub retired_at: Option<UtcTimestamp>,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The source test codes and the analytes they map to.
    pub test_codes: Vec<SourceTestCodeView>,
    /// The file template of a result-file source; `None` for an ASTM source
    /// (result files brief F2). For a file source the sender fields hold the
    /// declared software name and version.
    pub file_layout: Option<FileLayout>,
    /// Approved AU wire configuration and numeric units; absent for ASTM/files.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub au_profile: Option<crate::AuSourceProfile>,
    /// Engine-computed actions the UI may offer.
    pub allowed_actions: Vec<String>,
}

/// One source test code mapped to a catalog analyte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceTestCodeView {
    /// The manufacturer's local test code (R.3).
    pub source_test_code: String,
    /// The catalog analyte.
    pub analyte_id: AnalyteId,
}

/// A material lot. `expired` is derived from `expiresOn` at read time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LotView {
    /// The lot.
    pub lot_id: LotId,
    /// Reagent, calibrator, control or internal standard.
    pub material_kind: MaterialKind,
    /// The product name.
    pub product_name: String,
    /// The manufacturer's lot number.
    pub lot_number: String,
    /// The control or calibrator level, as labelled.
    pub level_text: Option<String>,
    /// The lab-defined sample label of a control or calibrator.
    pub sample_label: Option<String>,
    /// The last day of use, `YYYY-MM-DD`.
    pub expires_on: String,
    /// Whether the expiry date has passed (UTC date).
    pub expired: bool,
    /// Active or withdrawn.
    pub state: LotState,
    /// When it was recorded.
    pub recorded_at: UtcTimestamp,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// A preparation batch with its steps (REQ-ANA-001).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepBatchView {
    /// The batch.
    pub prep_batch_id: PrepBatchId,
    /// The method version it prepares for.
    pub method_version_id: MethodVersionId,
    /// Open while steps are recorded; completed batches never change.
    pub state: PrepBatchState,
    /// Who created it.
    pub created_by: String,
    /// When it was created.
    pub created_at: UtcTimestamp,
    /// Who completed it.
    pub completed_by: Option<String>,
    /// When it was completed.
    pub completed_at: Option<UtcTimestamp>,
    /// Optimistic-concurrency revision; each recorded step raises it.
    #[ts(type = "number")]
    pub revision: i64,
    /// The recorded steps, in order.
    pub steps: Vec<PrepStepView>,
}

/// One recorded preparation step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepStepView {
    /// The step.
    pub step_id: PrepStepId,
    /// Its order within the batch, from 1.
    #[ts(type = "number")]
    pub seq: i64,
    /// The lab-defined step code.
    pub step_code: String,
    /// The lot used, if any.
    pub lot_id: Option<LotId>,
    /// A note, as entered.
    pub note: Option<String>,
    /// Who performed it.
    pub performed_by: String,
    /// When it was recorded.
    pub performed_at: UtcTimestamp,
}

/// A run with lots, positions and patient/material context for an audited read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunDetail {
    /// Performing site pinned at run creation or resolved from retained evidence.
    pub laboratory_profile_id: Option<lis_identity::LaboratoryProfileId>,
    /// External evidence reference for a later factual resolution.
    pub performing_site_source: Option<String>,
    /// Reason supporting a later factual resolution.
    pub performing_site_reason: Option<String>,
    /// The run.
    pub run_id: RunId,
    /// The instrument.
    pub instrument_id: InstrumentId,
    /// The instrument's name.
    pub instrument_name: String,
    /// The method version.
    pub method_version_id: MethodVersionId,
    /// The method code.
    pub method_code: String,
    /// The method version number.
    #[ts(type = "number")]
    pub method_version: i64,
    /// Open or closed.
    pub state: RunState,
    /// Who created it.
    pub created_by: String,
    /// When it was created.
    pub created_at: UtcTimestamp,
    /// When it was closed.
    pub closed_at: Option<UtcTimestamp>,
    /// Why it was closed, as entered.
    pub close_reason: Option<String>,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The lots used by the run.
    pub lots: Vec<LotView>,
    /// The positions, in the order added.
    pub positions: Vec<RunPositionView>,
    /// Engine-computed actions the UI may offer.
    pub allowed_actions: Vec<String>,
}

/// Patient and material context for an explicitly opened run (audited by the engine).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunPatientContext {
    /// Accession containing this material.
    pub accession_id: lis_specimens::AccessionId,
    /// Human-readable accession number.
    pub accession_number: String,
    /// Supplied specimen matrix.
    pub specimen_type_code: String,
    /// Current patient identity and identifiers, never inferred from the sample label.
    pub subject: lis_identity::SubjectSummary,
}

/// One run position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunPositionView {
    /// Patient context for patient positions; absent for QC material.
    pub patient_context: Option<RunPatientContext>,
    /// The position.
    pub position_id: PositionId,
    /// The lab's position label on the run.
    pub label: String,
    /// Patient, control, calibrator or blank.
    pub role: PositionRole,
    /// The label the analyzer reports for this position.
    pub sample_label: String,
    /// The control or calibrator lot.
    pub lot_id: Option<LotId>,
    /// The patient execution, for a patient position.
    pub execution: Option<ExecutionView>,
}

/// A test execution of patient material (REQ-ANA-001, REQ-ANA-006).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecutionView {
    /// The execution.
    pub execution_id: ExecutionId,
    /// The container tested.
    pub container_id: ContainerId,
    /// Initial, rerun, re-extraction or dilution.
    pub attempt_kind: AttemptKind,
    /// The execution this attempt follows.
    pub related_execution_id: Option<ExecutionId>,
    /// The dilution factor, recorded only; the source reports final concentration.
    pub dilution_factor: Option<DecimalText>,
    /// The preparation batch.
    pub prep_batch_id: Option<PrepBatchId>,
    /// Why this attempt was made, as entered.
    pub reason: Option<String>,
    /// Active or failed.
    pub state: ExecutionState,
    /// Why it failed, as entered.
    pub state_reason: Option<String>,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The order components it serves.
    pub component_ids: Vec<OrderComponentId>,
    /// How many analytes those components expect.
    #[ts(type = "number")]
    pub expected_analytes: usize,
    /// Engine-computed actions the UI may offer.
    pub allowed_actions: Vec<String>,
}

/// A run in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunSummary {
    /// The run.
    pub run_id: RunId,
    /// The instrument.
    pub instrument_id: InstrumentId,
    /// The method version.
    pub method_version_id: MethodVersionId,
    /// Open or closed.
    pub state: RunState,
    /// When it was created.
    pub created_at: UtcTimestamp,
    /// How many positions it has.
    #[ts(type = "number")]
    pub position_count: i64,
}

/// A container with pending tests for a method version. No demographics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorklistEntry {
    /// The container.
    pub container_id: ContainerId,
    /// Its printed label code.
    pub label_code: String,
    /// The accession number.
    pub accession_number: String,
    /// The specimen type code.
    pub specimen_type_code: String,
    /// The pending order components the method serves.
    pub component_ids: Vec<OrderComponentId>,
    /// The codes of the orderables those components belong to.
    pub orderables: Vec<String>,
    /// When the specimen was received.
    pub received_at: UtcTimestamp,
}

/// The run position an imported sample label matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PositionMatch {
    /// No open run of the instrument has the label (`unknown_sample`).
    None,
    /// Exactly one position.
    One(PositionRef),
    /// Several open runs have it (`ambiguous_sample`); row order is never used.
    Ambiguous,
}

/// What an importer needs about a matched position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionRef {
    /// The position.
    pub position_id: PositionId,
    /// Its run.
    pub run_id: RunId,
    /// The role the LIS set before acquisition; it decides the route.
    pub role: PositionRole,
    /// The patient execution.
    pub execution_id: Option<ExecutionId>,
    /// The run's method version.
    pub method_version_id: MethodVersionId,
}

/// One execution of an order component, for billing's failed-attempt flag
/// (REQ-BIL-005): every execution failed means the work was attempted but
/// produced nothing billable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentExecution {
    /// The execution.
    pub execution_id: ExecutionId,
    /// Initial, rerun, re-extraction or dilution.
    pub attempt_kind: AttemptKind,
    /// Active or failed.
    pub state: ExecutionState,
}
