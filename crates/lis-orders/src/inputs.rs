//! IPC-safe command inputs. Text is preserved as supplied after bounded
//! validation; no clinical evidence is inferred, normalized or defaulted.

use lis_catalog::{OrderableId, OrderableVersionId};
use lis_identity::{ClientId, ClientLocationId, PractitionerId, SubjectId};
use serde::Deserialize;
use ts_rs::TS;

use crate::{
    AuthorityEvidenceKind, BillingResponsibility, BillingReviewElement, CancellationRequestId,
    ClarificationDeficiency, ClarificationHoldId, Decision, ItemKind, MedicationSource,
    OrderEvidenceId, OrderFilter, OrderId, OrderItemId, ReflexAuthorization, ResultRevisionId,
    SourceKind, StandingOrderId, TriggerSource,
};

/// Practice-template evidence; a template is never order authority by itself.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum TemplateUseInput {
    /// No clinic template was used.
    None,
    /// A named clinic template was used and its per-patient confirmation recorded.
    ClinicTemplate {
        /// Name exactly as supplied.
        name: String,
        /// Whether the source confirmed this patient's selection.
        #[serde(rename = "confirmedPerPatient")]
        confirmed_per_patient: bool,
    },
}

/// One requested orderable and its reflex authority as supplied.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OrderItemInput {
    /// Immutable catalog version selected by the source.
    pub orderable_version_id: OrderableVersionId,
    /// Clinical sequence role supplied by the ordering source (REQ-RES-002).
    pub sequence_role: crate::SequenceRole,
    /// Authority to activate reflex work.
    pub reflex_authorization: ReflexAuthorization,
    /// Logical orderables that authority permits as reflexes.
    pub reflex_scope_orderable_ids: Vec<OrderableId>,
}

/// Supplied authority evidence.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AuthorityEvidenceInput {
    /// Evidence kind, including an explicit `not_supplied` state.
    pub kind: AuthorityEvidenceKind,
    /// Source artifact reference, when supplied.
    pub document_ref: Option<String>,
}

/// Diagnosis/indication exactly as transcribed from a source artifact.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DiagnosisInput {
    pub code: String,
    pub code_system: String,
    pub code_system_version: Option<String>,
    pub document_ref: String,
    pub preprinted_on_lab_form: bool,
}

/// Medication statement exactly as supplied (REQ-ORD-007; INV-13).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MedicationInput {
    pub received_text: String,
    pub code: Option<String>,
    pub code_system: Option<String>,
    pub source: MedicationSource,
    pub reporter: Option<String>,
    pub provided_on: Option<String>,
    pub as_of_date: Option<String>,
}

/// Client point-of-care context. This never enters laboratory result tables.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PointOfCareResultInput {
    pub device_or_test: String,
    pub analyte_text: String,
    pub result_text: String,
    pub performed_at_text: Option<String>,
    pub source: String,
}

/// Evidence received for one order version.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct EvidenceInput {
    pub authority: AuthorityEvidenceInput,
    pub diagnoses: Vec<DiagnosisInput>,
    pub medications: Vec<MedicationInput>,
    pub point_of_care_results: Vec<PointOfCareResultInput>,
}

/// Registers an order as received (REQ-ORD-001, 005–007).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RegisterOrderInput {
    pub subject_id: SubjectId,
    pub client_id: ClientId,
    pub client_location_id: Option<ClientLocationId>,
    pub practitioner_id: PractitionerId,
    pub placer_order_id: Option<String>,
    pub placer_authority: Option<String>,
    pub source_kind: SourceKind,
    pub source_document_ref: Option<String>,
    pub billing_responsibility: BillingResponsibility,
    pub facility_setting: Option<String>,
    pub standing_order_id: Option<StandingOrderId>,
    pub template_use: TemplateUseInput,
    pub items: Vec<OrderItemInput>,
    pub evidence: EvidenceInput,
}

/// Gets one order by ID.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetOrderInput {
    pub order_id: OrderId,
}

/// Lists a bounded order worklist.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListOrdersInput {
    pub filter: OrderFilter,
    pub subject_id: Option<SubjectId>,
    pub limit: u32,
    pub before_received_at: Option<lis_core::UtcTimestamp>,
    /// Stable continuation through equal timestamps; preferred over timestamp-only legacy input.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub before_order_id: Option<OrderId>,
}

/// Adds and retires supplied evidence as a new order evidence version.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AmendOrderEvidenceInput {
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub add: EvidenceInput,
    pub remove_evidence_ids: Vec<OrderEvidenceId>,
    pub reason: String,
}

/// Accepts a received order after rechecking its gates.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AcceptOrderInput {
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub expected_revision: i64,
}

/// Rejects a received order.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RejectOrderInput {
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
}

/// Places an order-authority hold.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PlaceClarificationHoldInput {
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub deficiency: ClarificationDeficiency,
    pub note: Option<String>,
}

/// Resolves one open clarification hold.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolveClarificationHoldInput {
    pub hold_id: ClarificationHoldId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
}

/// Opens non-blocking payer-documentation review.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateBillingReviewItemInput {
    pub order_id: OrderId,
    pub element: BillingReviewElement,
    pub note: Option<String>,
}

/// Requests an add-on or reflex item.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RequestFollowOnInput {
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub kind: ItemKind,
    pub orderable_version_id: OrderableVersionId,
    /// Clinical role of this follow-on; item kind does not infer it.
    pub sequence_role: crate::SequenceRole,
    pub parent_item_id: Option<OrderItemId>,
    pub authority_reference: Option<String>,
    pub poc_result_id: Option<OrderEvidenceId>,
    pub trigger_source: Option<TriggerSource>,
    pub trigger_ref: Option<String>,
    pub reason: String,
}

/// Internal follow-on request from the results owner after result provenance is verified.
#[derive(Debug, Clone)]
pub struct LaboratoryResultFollowOnInput {
    pub order_id: OrderId,
    pub expected_revision: i64,
    pub orderable_version_id: OrderableVersionId,
    pub sequence_role: crate::SequenceRole,
    pub parent_item_id: OrderItemId,
    pub result_revision_id: ResultRevisionId,
    pub reason: String,
}

/// Accepts or declines a pending follow-on request.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DecideFollowOnInput {
    pub item_id: OrderItemId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub decision: Decision,
    pub reason: String,
}

/// Records a trigger decision that creates no item, charge or negative result.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordTriggerNotMetInput {
    pub parent_item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub trigger_source: TriggerSource,
    pub trigger_ref: String,
    pub reason: String,
}

/// Internal condition-not-met assessment from the results owner.
#[derive(Debug, Clone)]
pub struct LaboratoryResultTriggerNotMetInput {
    /// The order of the verified result revision; the parent item must belong to it.
    pub order_id: OrderId,
    pub parent_item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub result_revision_id: ResultRevisionId,
    pub reason: String,
}

/// Records a cancellation request without destroying work.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RequestCancellationInput {
    pub item_id: OrderItemId,
    pub requested_by: String,
    pub reason: String,
}

/// Accepts or declines a cancellation request.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DecideCancellationInput {
    pub request_id: CancellationRequestId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub decision: Decision,
    pub reason: String,
}

/// Registers a patient-specific standing order.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RegisterStandingOrderInput {
    pub subject_id: SubjectId,
    pub practitioner_id: PractitionerId,
    pub scope_orderable_ids: Vec<OrderableId>,
    pub valid_from: String,
    pub valid_until: String,
    pub visit_limit: Option<u32>,
    pub document_ref: String,
}

/// Lists standing orders for one subject.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListStandingOrdersInput {
    pub subject_id: SubjectId,
}
