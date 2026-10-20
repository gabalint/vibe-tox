//! Order-selected financial facts and review lifecycle, independent of clinical gates.

use lis_core::{Phi, UtcTimestamp};
use lis_identity::{
    BillingContact, BillingParty, ClientId, FinancialEvidence, PatientCoverage, SubjectId,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BillingResponsibility, BillingReviewElement, BillingReviewItemId, OrderId};

/// Snapshot of only the financial facts selected for this order (BILL-G02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OrderBillingSelection {
    /// Bill-to arrangement, including explicitly unresolved.
    pub responsibility: BillingResponsibility,
    /// Financially responsible client account, if selected.
    pub client_id: Option<ClientId>,
    /// Patient profile version used as the source, when selected.
    #[ts(type = "number | null")]
    pub profile_version: Option<i64>,
    /// Frozen patient billing address/contact.
    pub contact: Option<BillingContact>,
    /// Frozen selected responsible party.
    pub guarantor: Option<BillingParty>,
    /// Frozen selected coverages, in explicitly selected payment priority.
    pub coverages: Vec<PatientCoverage>,
}

/// One immutable, attributable order billing revision.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OrderBillingContext {
    /// Order this context describes.
    pub order_id: OrderId,
    /// Patient whose supplied facts were selected.
    pub subject_id: SubjectId,
    /// Consecutive order-local billing version.
    #[ts(type = "number")]
    pub version: i64,
    /// Order evidence version created by this financial correction.
    #[ts(type = "number")]
    pub order_version: i64,
    /// Frozen selected financial facts.
    pub selection: OrderBillingSelection,
    /// Manual evidence explicitly selected/recorded for this order.
    pub financial_evidence: Vec<FinancialEvidence>,
    /// Source for this confirmation.
    pub source: Phi<String>,
    /// Reason for this version.
    pub reason: Phi<String>,
    /// Person recording the confirmation.
    pub recorded_by: String,
    /// Engine recording time, never a backdated notice time.
    pub recorded_at: UtcTimestamp,
}

/// Selects immutable profile facts and creates/corrects an order billing context.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetOrderBillingContextInput {
    /// Order to correct; clinical scope is unchanged.
    pub order_id: OrderId,
    /// Current order revision from the order billing read model.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Current billing version, null when none exists.
    #[ts(type = "number | null")]
    pub expected_billing_version: Option<i64>,
    /// Explicit financial responsibility.
    pub responsibility: BillingResponsibility,
    /// Selected client bill-to, distinct from ordering client.
    pub client_id: Option<ClientId>,
    /// Patient profile revision to select from, or null.
    #[ts(type = "number | null")]
    pub profile_version: Option<i64>,
    /// Coverage keys in payment priority; no automatic selection.
    pub coverage_keys: Vec<String>,
    /// Whether the profile's guarantor was explicitly selected.
    pub use_profile_guarantor: bool,
    /// Actual manual evidence, including copied pre-order estimate evidence.
    pub financial_evidence: Vec<FinancialEvidence>,
    /// Information source for the order-specific confirmation.
    pub source: Phi<String>,
    /// Required reason for creating/correcting this billing record.
    pub reason: Phi<String>,
}

lis_core::text_enum! {
    /// Financial follow-up state, never a clinical testing/release gate.
    BillingReviewStatus { Open = "open", Resolved = "resolved", }
}

/// An effective review item, combining immutable opening and latest action.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingReviewRecord {
    /// Existing review item identifier.
    pub id: BillingReviewItemId,
    /// Related order.
    pub order_id: OrderId,
    /// Source-supplied order reference, when present; internal IDs stay out of labels.
    pub order_reference: Option<Phi<String>>,
    /// Financial evidence category needing review.
    pub element: BillingReviewElement,
    /// Original source note; never rewritten.
    pub note: Option<Phi<String>>,
    /// Opening time for ageing.
    pub opened_at: UtcTimestamp,
    /// Original recording actor.
    pub opened_by: String,
    /// Effective lifecycle version, starting at one.
    #[ts(type = "number")]
    pub version: i64,
    /// Current financial work state.
    pub status: BillingReviewStatus,
    /// Assigned person or team as supplied, or unassigned.
    pub owner: Option<String>,
    /// Last action's retained evidence reference.
    pub source_ref: Option<Phi<String>>,
    /// Last action's attributable explanation.
    pub reason: Option<Phi<String>>,
    /// Time of the latest lifecycle action, if any.
    pub updated_at: Option<UtcTimestamp>,
    /// Actor of the latest lifecycle action, if any.
    pub updated_by: Option<String>,
}

/// Assigns, resolves or reopens a financial review item with retained evidence.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UpdateBillingReviewItemInput {
    /// Item whose lifecycle changes.
    pub item_id: BillingReviewItemId,
    /// Last effective lifecycle version reviewed.
    #[ts(type = "number")]
    pub expected_version: i64,
    /// Proposed state; reopening preserves the earlier resolution.
    pub status: BillingReviewStatus,
    /// Responsible person/team; a resolution requires an owner.
    pub owner: Option<String>,
    /// Evidence for this action, required for resolution.
    pub source_ref: Option<Phi<String>>,
    /// Required action explanation.
    pub reason: Phi<String>,
}

/// Financial read model used by order entry, accessioning and billing.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderBillingView {
    /// Related order.
    pub order_id: OrderId,
    /// Patient identity authorized specifically for financial follow-up.
    pub subject: lis_identity::SubjectSummary,
    /// Current order revision for a financial correction.
    #[ts(type = "number")]
    pub order_revision: i64,
    /// Latest selected snapshot, or explicitly absent.
    pub context: Option<OrderBillingContext>,
    /// Static financial capture deficiencies; never clinical holds.
    pub readiness_issues: Vec<String>,
    /// Effective financial review work, including resolved history.
    pub review_items: Vec<BillingReviewRecord>,
    /// Commands allowed by the engine principal.
    pub allowed_actions: Vec<String>,
}

/// Bounded financial review queue, independent of clinical order worklists.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListBillingReviewItemsInput {
    /// Optional financial state filter; null includes both states.
    pub status: Option<BillingReviewStatus>,
    /// Maximum rows, 1–100, ordered oldest first.
    pub limit: u32,
    /// Continue after this retained item in stable opening-time/ID order.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after_item_id: Option<BillingReviewItemId>,
}
