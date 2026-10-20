//! Billing identifiers, states and read models (REQ-BIL-001/002/005).

use lis_catalog::{AnalyteId, MethodCertainty};
use lis_core::{Id, UserId, UtcTimestamp, text_enum};
use lis_identity::SubjectId;
use lis_orders::{OrderComponentId, OrderId};
use lis_reporting::ReportRevisionId;
use lis_specimens::SpecimenId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Marker for one analyte-to-class mapping version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingClassMappingVersion {}
/// One version of the lab-entered billing class mapping.
pub type BillingClassMappingVersionId = Id<BillingClassMappingVersion>;

/// Marker for one deterministic performed-service derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingDerivation {}
/// One immutable set of derived billing facts.
pub type BillingDerivationId = Id<BillingDerivation>;

/// Marker for one `bil-handoff-v1` file handed to the billing owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingHandoff {}
/// One immutable billing handoff.
pub type BillingHandoffId = Id<BillingHandoff>;

/// Marker for one derivation inside a handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingHandoffItem {}
/// One handoff item, whose outcome is recorded separately.
pub type BillingHandoffItemId = Id<BillingHandoffItem>;

text_enum! {
    /// Lifecycle of an analyte-to-class mapping version.
    BillingVersionStatus {
        /// Editable only by its author.
        Draft = "draft",
        /// Billing-approved; usable only after laboratory confirmation.
        Approved = "approved",
        /// Retained but unavailable for new derivations.
        Retired = "retired",
    }
}

text_enum! {
    /// Whether every specimen in a derivation has a current final report.
    DerivationCompleteness {
        /// Every specimen has a current final report.
        Complete = "complete",
        /// At least one specimen has not reached a current final report.
        AwaitingFinal = "awaiting_final",
    }
}

text_enum! {
    /// Review and handoff lifecycle of a service derivation.
    BillingDerivationState {
        /// Blocking billing data is absent; clinical work remains unaffected.
        Held = "held",
        /// Ready for billing-owner review.
        Candidate = "candidate",
        /// Approved for a controlled handoff.
        Approved = "approved",
        /// Included in an immutable handoff artifact.
        HandedOff = "handed_off",
        /// Replaced before handoff by a newer input digest.
        Superseded = "superseded",
    }
}

text_enum! {
    /// How an ordered component contributed to performed-service derivation.
    BillingComponentDisposition {
        /// At least one ordered analyte has a measured, currently reviewed result.
        Performed = "performed",
        /// The ordered component was cancelled.
        Cancelled = "cancelled",
        /// No measured, currently reviewed result exists.
        NotPerformed = "not_performed",
        /// Every execution failed and no successful performed work exists.
        FailedOnly = "failed_only",
        /// Specimen-validity work is excluded from class counts.
        ValidityExcluded = "validity_excluded",
    }
}

text_enum! {
    /// Whether a handoff item is new to the billing owner or replaces an earlier item.
    HandoffItemKind {
        /// The first item for its subject and date of service.
        Original = "original",
        /// Replaces the latest earlier item for the same subject and date of service.
        Adjustment = "adjustment",
    }
}

text_enum! {
    /// What the billing owner reported for a handoff item.
    HandoffOutcome {
        /// Received and accepted by the billing owner.
        Acknowledged = "acknowledged",
        /// Refused by the billing owner, with a reason.
        Rejected = "rejected",
    }
}

text_enum! {
    /// A handoff item's reconciliation status (`bil_v_handoff_status`).
    HandoffStatus {
        /// No outcome recorded yet.
        Outstanding = "outstanding",
        /// Acknowledged by the billing owner.
        Acknowledged = "acknowledged",
        /// Rejected by the billing owner.
        Rejected = "rejected",
    }
}

text_enum! {
    /// A clinical input reference frozen into a derivation digest.
    BillingInputKind {
        OrderVersion = "order_version",
        BillingContext = "billing_context",
        BillingReview = "billing_review",
        CollectionRevision = "collection_revision",
        ConflictResolution = "conflict_resolution",
        ComponentState = "component_state",
        ResultRevision = "result_revision",
        Review = "review",
        ReportRevision = "report_revision",
        Execution = "execution",
        QuarantineItem = "quarantine_item",
    }
}

/// One lab-entered mapping entry. It never carries payer or procedure-code content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingClassMappingEntry {
    pub analyte_id: AnalyteId,
    pub class_code: String,
    pub class_label: String,
}

/// Lifecycle summary of one mapping version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingClassMappingVersionSummary {
    pub version_id: BillingClassMappingVersionId,
    #[ts(type = "number")]
    pub version: i64,
    pub status: BillingVersionStatus,
    #[ts(type = "number")]
    pub revision: i64,
    pub created_by: UserId,
    /// Current account display name, resolved by the engine at read time.
    #[ts(optional)]
    pub created_by_display: Option<String>,
    pub created_at: UtcTimestamp,
    pub approved_by: Option<UserId>,
    /// Current approver display name, resolved by the engine at read time.
    #[ts(optional)]
    pub approved_by_display: Option<String>,
    pub approved_at: Option<UtcTimestamp>,
    pub confirmed_by: Option<UserId>,
    pub confirmed_at: Option<UtcTimestamp>,
    pub effective_from: Option<UtcTimestamp>,
    pub effective_to: Option<UtcTimestamp>,
    pub retired_by: Option<UserId>,
    pub retired_at: Option<UtcTimestamp>,
    pub retire_reason: Option<String>,
}

/// One mapping version with its immutable content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingClassMappingVersionView {
    #[serde(flatten)]
    pub summary: BillingClassMappingVersionSummary,
    pub entries: Vec<BillingClassMappingEntry>,
    /// The lifecycle commands the version's state permits; authorship and
    /// capability are still checked by each command.
    pub allowed_actions: Vec<String>,
}

/// A class identity kept with the derivation so later mapping edits cannot rewrite it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct BillingClass {
    pub code: String,
    pub label: String,
}

/// Ordered, performed and reported membership for one specimen and certainty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassFact {
    pub specimen_id: SpecimenId,
    pub class: BillingClass,
    pub certainty: MethodCertainty,
    pub ordered: bool,
    pub performed: bool,
    pub reported: bool,
}

/// A billing review flag. Blocking means only that the derivation is held;
/// no flag ever blocks testing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingFlag {
    pub code: String,
    pub ref_id: String,
    pub blocking: bool,
}

/// Persisted component disposition for one specimen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedComponent {
    pub component_id: OrderComponentId,
    pub specimen_id: SpecimenId,
    pub disposition: BillingComponentDisposition,
}

/// Persisted specimen lineage for one derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedSpecimen {
    pub specimen_id: SpecimenId,
    pub order_id: OrderId,
    pub order_version: i64,
    pub report_revision_id: Option<ReportRevisionId>,
}

/// Immutable facts produced from one canonical input digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivationFacts {
    pub subject_id: SubjectId,
    pub date_of_service: Option<String>,
    pub dos_rule: String,
    pub mapping_version_id: Option<BillingClassMappingVersionId>,
    pub completeness: DerivationCompleteness,
    pub state: BillingDerivationState,
    pub specimens: Vec<DerivedSpecimen>,
    pub components: Vec<DerivedComponent>,
    pub classes: Vec<ClassFact>,
    pub flags: Vec<BillingFlag>,
}
