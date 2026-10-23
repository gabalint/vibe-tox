//! Read models returned to the engine and the UI. No demographics: the
//! subject is referenced by ID only.

use lis_core::{DecimalText, UtcTimestamp};
use lis_identity::{SubjectId, SubjectSummary};
use lis_orders::{OrderId, OrderItemId};
use serde::Serialize;
use ts_rs::TS;

use crate::model::{
    AccessionId, CollectionId, CollectionObservationId, CollectionPrecision, ContainerHoldKind,
    ContainerHoldState, ContainerId, ContainerKind, ContainerState, ExcursionId, ExcursionOutcome,
    ExcursionState, HoldId, LabelJobId, LabelJobKind, LabelOutcome, LocationId, PrinterId,
    PrinterTransport, ReceiptDecision, ReceiptDecisionId, ReceiptDiscrepancyCode,
    SpecimenAcceptance, SpecimenId, VolumeKind,
};

/// An accession with its collection, specimens and containers (REQ-SPC-001).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccessionDetail {
    /// The accession.
    pub accession_id: AccessionId,
    /// Lab accession number, such as `A26000001`.
    pub accession_number: String,
    /// The order it fulfils.
    pub order_id: OrderId,
    /// The order items it fulfils.
    pub order_item_ids: Vec<OrderItemId>,
    /// The subject, from the order.
    pub subject_id: SubjectId,
    /// When it was created.
    pub created_at: UtcTimestamp,
    /// The collection event.
    pub collection: CollectionView,
    /// Specimens with their containers.
    pub specimens: Vec<AccessionSpecimenView>,
}

/// A collection event as supplied (REQ-SPC-002, REQ-SPC-008).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionView {
    /// The collection.
    pub collection_id: CollectionId,
    /// Datetime, date only, or unknown.
    pub precision: CollectionPrecision,
    /// The collection instant in UTC, for `datetime`.
    pub collected_at: Option<UtcTimestamp>,
    /// The source's UTC offset in minutes, for `datetime`.
    pub utc_offset_minutes: Option<i32>,
    /// The collection date, for `date`.
    pub collected_date: Option<String>,
    /// Who collected, as supplied.
    pub collector: Option<String>,
    /// Collection-site observations.
    pub observations: Vec<CollectionObservationView>,
}

/// One collection-site observation, attributed to its observer (REQ-SPC-008).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionObservationView {
    /// The observation.
    pub observation_id: CollectionObservationId,
    /// Lab-defined observation code.
    pub code: String,
    /// The value as reported.
    pub value_text: String,
    /// Who observed it, as reported.
    pub observed_by: String,
}

/// A specimen with its receipt decisions and containers.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccessionSpecimenView {
    /// The specimen.
    pub specimen_id: SpecimenId,
    /// Its matrix.
    pub specimen_type_code: String,
    /// Current acceptance.
    pub acceptance: SpecimenAcceptance,
    /// When it was received.
    pub received_at: UtcTimestamp,
    /// Optimistic-concurrency revision for receipt decisions.
    #[ts(type = "number")]
    pub revision: i64,
    /// Receipt decisions, oldest first.
    pub decisions: Vec<ReceiptDecisionView>,
    /// Primary containers and aliquots, in creation order.
    pub containers: Vec<ContainerView>,
    /// Commands the specimen's state allows now.
    pub allowed_actions: Vec<String>,
}

/// One receipt decision (REQ-SPC-002).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReceiptDecisionView {
    /// The decision record.
    pub decision_id: ReceiptDecisionId,
    /// What was decided.
    pub decision: ReceiptDecision,
    /// Documented discrepancies.
    pub discrepancies: Vec<ReceiptDiscrepancyView>,
    /// The lab policy permitting a conditional acceptance.
    pub policy_reference: Option<String>,
    /// The scope of a conditional acceptance.
    pub scope: Option<String>,
    /// Why, as entered.
    pub reason: Option<String>,
    /// Who decided (user ID).
    pub decided_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub decided_by_display: Option<String>,
    /// When.
    pub decided_at: UtcTimestamp,
    /// Committed aggregate revision; absent when legacy chronology needs review.
    #[ts(type = "number | null")]
    pub causal_sequence: Option<i64>,
}

/// One documented discrepancy.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReceiptDiscrepancyView {
    /// The discrepancy.
    pub code: ReceiptDiscrepancyCode,
    /// A note, as entered.
    pub note: Option<String>,
}

/// A container or aliquot with its location, open holds and label jobs.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerView {
    /// The container.
    pub container_id: ContainerId,
    /// The printed label code, such as `C00000001`; never changes.
    pub label_code: String,
    /// The specimen it holds.
    pub specimen_id: SpecimenId,
    /// Primary or aliquot.
    pub kind: ContainerKind,
    /// The parent of an aliquot.
    pub parent_container_id: Option<ContainerId>,
    /// Lifecycle state.
    pub state: ContainerState,
    /// The volume received, or taken from the parent for an aliquot.
    pub volume: Option<DecimalText>,
    /// Its unit text.
    pub volume_unit: Option<String>,
    /// Estimated, measured or unknown.
    pub volume_kind: VolumeKind,
    /// The current storage location, if it was ever moved.
    pub location: Option<StorageLocationRef>,
    /// Earlier move order cannot be established; record an evidenced current move.
    pub location_chronology_unresolved: bool,
    /// Open holds.
    pub open_holds: Vec<ContainerHoldView>,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// Label jobs, oldest first.
    pub label_jobs: Vec<LabelJobView>,
    /// Commands the container's state allows now.
    pub allowed_actions: Vec<String>,
}

/// A storage location reference.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StorageLocationRef {
    /// The location.
    pub location_id: LocationId,
    /// Its code.
    pub code: String,
    /// Its name.
    pub name: String,
}

/// A hold on a container (REQ-SPC-005).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerHoldView {
    /// The hold.
    pub hold_id: HoldId,
    /// The container.
    pub container_id: ContainerId,
    /// Why it is held.
    pub kind: ContainerHoldKind,
    /// Open or released.
    pub state: ContainerHoldState,
    /// The excursion that caused it, for excursion holds.
    pub excursion_id: Option<ExcursionId>,
    /// Why, as entered.
    pub reason: String,
    /// When it was placed.
    pub placed_at: UtcTimestamp,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// One label job. `handedOff` means the bytes reached the printer, not that
/// a label printed; no outcome yet means queued.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LabelJobView {
    /// The job.
    pub job_id: LabelJobId,
    /// The container.
    pub container_id: ContainerId,
    /// The printer.
    pub printer_id: PrinterId,
    /// Print or reprint.
    pub kind: LabelJobKind,
    /// When it was requested.
    pub requested_at: UtcTimestamp,
    /// The outcome, once the worker has recorded one.
    pub outcome: Option<LabelOutcome>,
    /// A PHI-free error code for a failed job.
    pub error_code: Option<String>,
}

/// A container found by its label, with its accession and subject.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerLookup {
    /// The container.
    pub container: ContainerView,
    /// Its specimen's matrix.
    pub specimen_type_code: String,
    /// Its specimen's acceptance.
    pub acceptance: SpecimenAcceptance,
    /// The accession.
    pub accession_id: AccessionId,
    /// The accession number.
    pub accession_number: String,
    /// The subject's current demographics and identifiers, so the label can
    /// be checked against the person (PHI: an audited read).
    pub subject: SubjectSummary,
}

/// A storage location (configuration).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StorageLocationView {
    /// The location.
    pub location_id: LocationId,
    /// Unique code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Storage condition, as configured.
    pub storage_condition: Option<String>,
    /// Whether containers may be moved here.
    pub active: bool,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// A label printer (configuration).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LabelPrinterView {
    /// The printer.
    pub printer_id: PrinterId,
    /// Unique code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// TCP or CUPS.
    pub transport: PrinterTransport,
    /// IP address, for TCP.
    pub ip_address: Option<String>,
    /// Port, for TCP.
    pub port: Option<u16>,
    /// Queue name, for CUPS.
    pub queue: Option<String>,
    /// Whether labels may be sent to it.
    pub active: bool,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// A storage excursion and its assessment (REQ-SPC-005).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExcursionView {
    /// The excursion.
    pub excursion_id: ExcursionId,
    /// Where it happened.
    pub location_id: LocationId,
    /// Start of the observed excursion.
    pub observed_from: UtcTimestamp,
    /// End, if known.
    pub observed_to: Option<UtcTimestamp>,
    /// What was observed, as entered.
    pub description: String,
    /// Open or assessed.
    pub state: ExcursionState,
    /// The assessment outcome.
    pub outcome: Option<ExcursionOutcome>,
    /// Holds placed by the assessment.
    pub holds: Vec<ContainerHoldView>,
    /// Stable barcode links for every container named by an assessment.
    pub affected_container_labels: std::collections::BTreeMap<String, String>,
    pub recorded_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub recorded_by_display: Option<String>,
    pub recorded_at: UtcTimestamp,
    pub assessed_by: Option<String>,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub assessed_by_display: Option<String>,
    pub assessed_at: Option<UtcTimestamp>,
    pub assessment_reason: Option<String>,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// A container that may be placed on an analytical run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerForTesting {
    /// The container.
    pub container_id: ContainerId,
    /// Its printed label code, the only patient-material token on a run.
    pub label_code: String,
    /// Its specimen.
    pub specimen_id: SpecimenId,
    /// The accession.
    pub accession_id: AccessionId,
    /// The subject.
    pub subject_id: SubjectId,
}

/// A hold with its release time, for history after a report's release
/// (REQ-RPT-002 impact): a released hold still counts.
#[derive(Debug, Clone)]
pub struct ContainerHoldRecord {
    /// The hold as shown on the container.
    pub hold: ContainerHoldView,
    /// When it was released; `None` while open.
    pub released_at: Option<UtcTimestamp>,
}

/// One specimen of a subject, with its accession and collection time as
/// recorded (REQ-BIL-005 date of service). No demographics.
#[derive(Debug, Clone)]
pub struct SubjectSpecimen {
    /// The specimen.
    pub specimen_id: SpecimenId,
    /// Its matrix.
    pub specimen_type_code: String,
    /// Current acceptance.
    pub acceptance: SpecimenAcceptance,
    /// When it was received.
    pub received_at: UtcTimestamp,
    /// The accession.
    pub accession_id: AccessionId,
    /// The accession number.
    pub accession_number: String,
    /// The order the accession fulfils.
    pub order_id: OrderId,
    /// Datetime, date only, or unknown.
    pub precision: CollectionPrecision,
    /// The collection instant in UTC, for `datetime`.
    pub collected_at: Option<UtcTimestamp>,
    /// The source's UTC offset in minutes, for `datetime`.
    pub utc_offset_minutes: Option<i32>,
    /// The collection date, for `date`.
    pub collected_date: Option<String>,
}
