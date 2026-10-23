//! Read models for order entry. Supplied clinical text appears only in these
//! clinical records, never in audit details or diagnostic logs.

use lis_catalog::{ComponentPurpose, OrderableId, OrderableVersionId};
use lis_core::{Phi, UtcTimestamp};
use lis_identity::{ClientId, ClientLocationId, PractitionerId, SubjectId};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    AuthorityEvidenceKind, BillingResponsibility, BillingReviewElement, BillingReviewItemId,
    CancellationRequestId, ClarificationDeficiency, ClarificationHoldId, ComponentState, ItemKind,
    ItemState, MedicationSource, OrderComponentId, OrderEvidenceId, OrderId, OrderItemId,
    OrderStatus, ReflexAuthorization, SequenceRole, SourceKind, StandingOrderId,
    TriggerAssessmentId, TriggerSource,
};

/// Frozen component of an order item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderComponentView {
    pub component_id: OrderComponentId,
    #[ts(type = "number")]
    pub catalog_component_position: i64,
    pub specimen_type_code: String,
    pub purpose: ComponentPurpose,
    pub sequence_role: SequenceRole,
    pub state: ComponentState,
    #[ts(type = "number")]
    pub revision: i64,
}

/// One initial, add-on or reflex order item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderItemView {
    pub item_id: OrderItemId,
    pub kind: ItemKind,
    pub state: ItemState,
    pub orderable_version_id: OrderableVersionId,
    /// Code resolved through the ordered version, including retired versions.
    pub orderable_code: String,
    /// Name of the immutable catalog orderable referenced by the ordered version.
    pub orderable_name: String,
    /// `None` only for a requested item migrated from schema v2.
    pub sequence_role: Option<SequenceRole>,
    pub parent_item_id: Option<OrderItemId>,
    pub reflex_authorization: ReflexAuthorization,
    pub reflex_scope_orderable_ids: Vec<OrderableId>,
    pub authority_reference: Option<String>,
    pub reason: Option<String>,
    #[ts(type = "number")]
    pub revision: i64,
    pub components: Vec<OrderComponentView>,
}

/// Authority evidence with version lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorityEvidenceView {
    pub evidence_id: OrderEvidenceId,
    pub kind: AuthorityEvidenceKind,
    pub document_ref: Option<String>,
    #[ts(type = "number")]
    pub added_in_version: i64,
    #[ts(type = "number | null")]
    pub removed_in_version: Option<i64>,
}

/// Diagnosis exactly as received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiagnosisView {
    pub evidence_id: OrderEvidenceId,
    pub code: String,
    pub code_system: String,
    pub code_system_version: Option<String>,
    pub document_ref: String,
    pub preprinted_on_lab_form: bool,
    #[ts(type = "number")]
    pub added_in_version: i64,
    #[ts(type = "number | null")]
    pub removed_in_version: Option<i64>,
}

/// Medication context exactly as received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MedicationView {
    pub evidence_id: OrderEvidenceId,
    pub received_text: String,
    pub code: Option<String>,
    pub code_system: Option<String>,
    pub source: MedicationSource,
    pub reporter: Option<String>,
    pub provided_on: Option<String>,
    pub as_of_date: Option<String>,
    #[ts(type = "number")]
    pub added_in_version: i64,
    #[ts(type = "number | null")]
    pub removed_in_version: Option<i64>,
}

/// Client point-of-care context; never a laboratory result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PointOfCareResultView {
    pub evidence_id: OrderEvidenceId,
    pub device_or_test: String,
    pub analyte_text: String,
    pub result_text: String,
    pub performed_at_text: Option<String>,
    pub source: String,
    #[ts(type = "number")]
    pub added_in_version: i64,
    #[ts(type = "number | null")]
    pub removed_in_version: Option<i64>,
}

/// Evidence grouped for an order detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderEvidenceView {
    pub authority: Vec<AuthorityEvidenceView>,
    pub diagnoses: Vec<DiagnosisView>,
    pub medications: Vec<MedicationView>,
    pub point_of_care_results: Vec<PointOfCareResultView>,
}

/// One clarification hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClarificationHoldView {
    pub hold_id: ClarificationHoldId,
    pub deficiency: ClarificationDeficiency,
    pub origin: String,
    pub state: String,
    pub note: Option<String>,
    pub opened_at: UtcTimestamp,
    pub resolved_at: Option<UtcTimestamp>,
    pub resolution_reason: Option<String>,
    #[ts(type = "number")]
    pub revision: i64,
}

/// Non-blocking billing review item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingReviewItemView {
    pub item_id: BillingReviewItemId,
    pub element: BillingReviewElement,
    pub note: Option<String>,
    pub opened_at: UtcTimestamp,
}

/// A cancellation request and decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CancellationView {
    pub request_id: CancellationRequestId,
    pub item_id: OrderItemId,
    pub requested_by: String,
    pub reason: String,
    pub state: String,
    pub decision_reason: Option<String>,
    #[ts(type = "number")]
    pub revision: i64,
}

/// An explicit trigger assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TriggerAssessmentView {
    pub assessment_id: TriggerAssessmentId,
    pub parent_item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub outcome: String,
    pub trigger_source: TriggerSource,
    pub trigger_ref: String,
    pub created_item_id: Option<OrderItemId>,
    pub reason: String,
    pub decided_by: String,
    pub decided_at: UtcTimestamp,
}

/// Complete order-entry record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderDetail {
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub version: i64,
    #[ts(type = "number")]
    pub revision: i64,
    pub status: OrderStatus,
    pub subject_id: SubjectId,
    pub practitioner_id: PractitionerId,
    pub client_id: ClientId,
    pub client_location_id: Option<ClientLocationId>,
    pub placer_order_id: Option<String>,
    pub placer_authority: Option<String>,
    pub source_kind: SourceKind,
    pub source_document_ref: Option<String>,
    pub billing_responsibility: BillingResponsibility,
    pub facility_setting: Option<String>,
    pub standing_order_id: Option<StandingOrderId>,
    pub template_name: Option<String>,
    pub template_confirmed_per_patient: Option<bool>,
    pub received_at: UtcTimestamp,
    pub items: Vec<OrderItemView>,
    pub evidence: OrderEvidenceView,
    pub holds: Vec<ClarificationHoldView>,
    pub billing_review_items: Vec<BillingReviewItemView>,
    pub cancellations: Vec<CancellationView>,
    pub trigger_assessments: Vec<TriggerAssessmentView>,
    pub allowed_actions: Vec<String>,
}

/// One order in the worklist, with what a person needs to recognize it without opening
/// it (REQ-ORD-001). Names come from the identity module as recorded; PHI, so the read
/// is audited by the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderSummary {
    pub order_id: OrderId,
    pub subject_id: SubjectId,
    /// Family name of the patient's current identity assertion.
    pub patient_family_name: Phi<String>,
    /// Given names of the patient's current identity assertion.
    pub patient_given_names: Option<Phi<String>>,
    pub patient_birth_date: Phi<String>,
    pub patient_identifiers: Vec<lis_identity::IdentifierView>,
    /// The ordering source's order number, as supplied.
    pub placer_order_id: Option<String>,
    /// Display name of the ordering client organization.
    pub client_name: String,
    /// Label codes of the order's primary sample containers, in accession order;
    /// filled by the engine from the specimens module, empty before receipt.
    pub sample_barcodes: Vec<String>,
    /// Items on the order in any state: requested, add-on and reflex items included.
    #[ts(type = "number")]
    pub item_count: i64,
    pub status: OrderStatus,
    pub received_at: UtcTimestamp,
    #[ts(type = "number")]
    pub version: i64,
    #[ts(type = "number")]
    pub revision: i64,
    #[ts(type = "number")]
    pub open_hold_count: i64,
}

/// Patient-specific standing-order record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StandingOrderView {
    /// Accepted orders reserve one use, including later cancelled work.
    pub accepted_uses: u32,
    pub standing_order_id: StandingOrderId,
    pub subject_id: SubjectId,
    pub practitioner_id: PractitionerId,
    pub valid_from: String,
    pub valid_until: String,
    pub visit_limit: Option<u32>,
    pub document_ref: String,
    pub recorded_at: UtcTimestamp,
    pub scope_orderable_ids: Vec<OrderableId>,
}
