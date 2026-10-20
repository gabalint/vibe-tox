//! Reusable financial information, supplied rather than inferred (BILL-G01/G07).

use lis_core::{DecimalText, Phi, UtcTimestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::SubjectId;

lis_core::text_enum! {
    /// The patient's relationship to the named subscriber or responsible party.
    BillingRelationship {
        Self_ = "self", Spouse = "spouse", Child = "child", Parent = "parent",
        Other = "other", Unknown = "unknown",
    }
}

/// A supplied postal address and contact channels; absence remains explicit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BillingContact {
    /// Street/address lines, at most three.
    pub address_lines: Vec<Phi<String>>,
    /// City or locality.
    pub city: Option<Phi<String>>,
    /// State/province as supplied.
    pub region: Option<Phi<String>>,
    /// Postal code as supplied.
    pub postal_code: Option<Phi<String>>,
    /// Country as supplied; never defaulted.
    pub country: Option<String>,
    /// Telephone as supplied.
    pub phone: Option<Phi<String>>,
    /// Email as supplied.
    pub email: Option<Phi<String>>,
}

/// A financial party, distinct from the patient and from report-access authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BillingParty {
    /// Name exactly as supplied.
    pub name: Phi<String>,
    /// Patient's relationship to this party, not the inverse.
    pub relationship: BillingRelationship,
    /// Known birth date, YYYY-MM-DD.
    pub birth_date: Option<Phi<String>>,
    /// Contact details for this party.
    pub contact: BillingContact,
}

/// One coverage candidate; its presence never establishes service eligibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PatientCoverage {
    /// Stable, patient-local key retained across profile revisions.
    pub key: String,
    /// Payer's name.
    pub payer_name: String,
    /// Identifier assigned by the receiving billing partner, if known.
    pub payer_id: Option<String>,
    /// Plan name, if supplied.
    pub plan_name: Option<String>,
    /// Member/subscriber identifier; unknown is not fabricated.
    pub member_id: Option<Phi<String>>,
    /// Group identifier, when applicable.
    pub group_number: Option<Phi<String>>,
    /// Subscriber as supplied, distinct from guarantor.
    pub subscriber: Option<BillingParty>,
    /// First applicable date, or unknown.
    pub effective_from: Option<String>,
    /// Last applicable date, or unknown.
    pub effective_until: Option<String>,
    /// Origin of this information, such as a retained document reference.
    pub source: Phi<String>,
    /// Date the supplied details were verified; not an eligibility guarantee.
    pub verified_on: Option<String>,
}

lis_core::text_enum! {
    /// Manually recorded financial evidence; no automatic payer decision.
    FinancialEvidenceKind {
        Eligibility = "eligibility", PriorAuthorization = "prior_authorization",
        AdvanceNotice = "advance_notice", GoodFaithEstimate = "good_faith_estimate",
        ProviderEnrollment = "provider_enrollment",
    }
}

lis_core::text_enum! {
    /// The source's explicit outcome, separate from payment approval.
    FinancialEvidenceStatus {
        NotChecked = "not_checked", Unknown = "unknown", NotRequired = "not_required",
        Pending = "pending", Recorded = "recorded", Approved = "approved", Denied = "denied",
    }
}

/// Details of an actual notice/estimate; a late record does not backdate notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BillingNoticeEvidence {
    /// Form/version as supplied.
    pub form_version: Option<String>,
    /// Stated noncoverage reason or estimate context.
    pub reason: Phi<String>,
    /// Exact amount, without floating-point conversion.
    pub estimated_amount: Option<DecimalText>,
    /// Supplied currency code when an amount is recorded.
    pub currency: Option<String>,
    /// Patient's choice as recorded on the retained notice.
    pub choice: Option<Phi<String>>,
    /// Actual delivery time, when known.
    pub provided_at: Option<UtcTimestamp>,
    /// Actual signature time, when known.
    pub signed_at: Option<UtcTimestamp>,
    /// Signer as supplied; not a report-access grant.
    pub signer: Option<Phi<String>>,
}

/// A bounded manual evidence record; external integrations are outside this scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FinancialEvidence {
    /// Stable key within this financial record.
    pub key: String,
    /// Evidence category.
    pub kind: FinancialEvidenceKind,
    /// Outcome reported by the source.
    pub status: FinancialEvidenceStatus,
    /// Retained document or external record reference.
    pub source_ref: Phi<String>,
    /// Services/request covered by the evidence, exactly as supplied.
    pub scope: Phi<String>,
    /// When the check/request/notice actually occurred.
    pub occurred_at: UtcTimestamp,
    /// First applicable service date, if known.
    pub valid_from: Option<String>,
    /// Last applicable service date, if known.
    pub valid_until: Option<String>,
    /// Payer/authentication/estimate reference as supplied.
    pub reference_number: Option<Phi<String>>,
    /// Selected coverage key, when applicable.
    pub coverage_key: Option<String>,
    /// Ordering provider for enrollment evidence, when known.
    pub provider_npi: Option<String>,
    /// Authorized units as supplied, when applicable.
    pub units: Option<u32>,
    /// Actual notice details, when supplied.
    pub notice: Option<BillingNoticeEvidence>,
    /// Additional source facts, never a generated coverage conclusion.
    pub notes: Option<Phi<String>>,
}

/// Full replacement content of a versioned patient financial profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PatientBillingContent {
    /// Patient's billing/contact details.
    pub contact: BillingContact,
    /// Reusable responsible party, explicitly supplied.
    pub guarantor: Option<BillingParty>,
    /// Coverage history/candidates; order selection is separate.
    pub coverages: Vec<PatientCoverage>,
    /// Evidence that can predate an accepted order, including estimate requests.
    pub financial_evidence: Vec<FinancialEvidence>,
}

/// One immutable version of a patient's financial profile.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PatientBillingProfile {
    /// Patient this profile belongs to.
    pub subject_id: SubjectId,
    /// Consecutive patient-local financial version.
    #[ts(type = "number")]
    pub version: i64,
    /// Supplied financial facts.
    pub content: PatientBillingContent,
    /// Source for the profile confirmation.
    pub source: Phi<String>,
    /// Why this version was recorded.
    pub reason: Phi<String>,
    /// Person who recorded the version.
    pub recorded_by: String,
    /// Actual recording time from the engine clock.
    pub recorded_at: UtcTimestamp,
}

/// Creates the first profile or corrects it with optimistic concurrency.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetPatientBillingProfileInput {
    /// Patient whose financial information is being recorded.
    pub subject_id: SubjectId,
    /// Current financial version, null for the first.
    #[ts(type = "number | null")]
    pub expected_version: Option<i64>,
    /// Replacement content; previous versions remain retained.
    pub content: PatientBillingContent,
    /// Information source, retained without entering diagnostics.
    pub source: Phi<String>,
    /// Attributable change reason.
    pub reason: Phi<String>,
}

/// Patient finance query arguments; historical versions remain retrievable.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetPatientBillingProfileInput {
    /// Patient to retrieve.
    pub subject_id: SubjectId,
    /// Exact historical version, or latest.
    #[ts(type = "number | null")]
    pub version: Option<i64>,
}

/// Audited patient finance read model with engine-derived actions.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PatientBillingProfileView {
    /// Patient queried.
    pub subject_id: SubjectId,
    /// Identity for this authorized financial view, without a separate clinical read.
    pub subject: crate::SubjectSummary,
    /// Selected profile or explicit absence.
    pub profile: Option<PatientBillingProfile>,
    /// Commands allowed to this principal, convenience only.
    pub allowed_actions: Vec<String>,
}
