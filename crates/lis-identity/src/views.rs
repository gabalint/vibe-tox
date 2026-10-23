//! Read models returned by commands and queries. PHI fields stay in `Phi`,
//! so `Debug` output redacts them; serialization to the UI is explicit.

use lis_core::{Phi, UtcTimestamp};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use serde::Serialize;
use ts_rs::TS;

use crate::model::{
    ClientId, ClientLocationId, IdentityAssertionId, IdentityHoldId, LaboratoryProfileId, Npi,
    PractitionerId, SubjectId, SubjectIdentifierId,
};

/// A subject with every assertion (oldest first) and every identifier.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectView {
    /// The subject.
    pub id: SubjectId,
    /// Optimistic-concurrency revision for subject commands.
    #[ts(type = "number")]
    pub revision: i64,
    /// Demographic assertions in sequence order; the last is the current one.
    pub assertions: Vec<AssertionView>,
    /// Active and inactive identifiers, oldest first.
    pub identifiers: Vec<IdentifierView>,
    /// Open identity holds on the subject or listing it as a candidate.
    pub open_holds: Vec<IdentityHoldView>,
}

/// One demographic assertion as provided.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "IdentityAssertionView")]
pub struct AssertionView {
    /// The assertion.
    pub id: IdentityAssertionId,
    /// Position within the subject's assertions, from 1.
    #[ts(type = "number")]
    pub sequence: i64,
    /// Family name as provided.
    pub family_name: Phi<String>,
    /// Given names as provided.
    pub given_names: Option<Phi<String>>,
    /// Birth date as provided (`YYYY-MM-DD`).
    pub birth_date: Phi<String>,
    /// Administrative sex as provided.
    pub administrative_sex: Option<Phi<String>>,
    /// Where the demographics came from.
    pub source: String,
    /// Why the demographics changed; absent on the first assertion.
    pub reason: Option<String>,
    /// When the assertion was recorded.
    pub asserted_at: UtcTimestamp,
    /// `user`, `instrument` or `system`.
    pub asserted_by_kind: String,
    /// The acting user, instrument or component.
    pub asserted_by_id: String,
}

/// One identifier, scoped by assigning authority and type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "SubjectIdentifierView")]
pub struct IdentifierView {
    /// The identifier row.
    pub id: SubjectIdentifierId,
    /// The assigning authority.
    pub assigning_authority: String,
    /// The identifier type code.
    pub identifier_type: String,
    /// The value.
    pub value: Phi<String>,
    /// Whether the identifier is active (unique within authority and type).
    pub active: bool,
    /// When it was added.
    pub added_at: UtcTimestamp,
    /// When it was deactivated.
    pub deactivated_at: Option<UtcTimestamp>,
    /// Why it was deactivated.
    pub deactivation_reason: Option<String>,
}

/// Subjects that match a query: identifier matches are the same subject;
/// candidates only share name and birth date and are never linked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectMatches {
    /// Subjects holding one of the active identifiers (more than one is a conflict to resolve).
    pub identifier_matches: Vec<SubjectId>,
    /// Other subjects with the same normalized name and birth date.
    pub candidates: Vec<SubjectId>,
}

/// Why an identity hold exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "IdentityHoldKind")]
pub enum HoldKind {
    /// The subject was registered while name and birth date candidates existed.
    PossibleDuplicate,
}

impl HoldKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PossibleDuplicate => "possible_duplicate",
        }
    }
}

impl FromSql for HoldKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "possible_duplicate" => Ok(Self::PossibleDuplicate),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// An open identity hold. It blocks release for its subject and every candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IdentityHoldView {
    /// The hold.
    pub id: IdentityHoldId,
    /// Optimistic-concurrency revision for resolving it.
    #[ts(type = "number")]
    pub revision: i64,
    /// Why it exists.
    pub kind: HoldKind,
    /// The subject registered while candidates existed.
    pub subject_id: SubjectId,
    /// The existing subjects it may duplicate.
    pub candidate_subject_ids: Vec<SubjectId>,
    /// When it was opened.
    pub opened_at: UtcTimestamp,
}

/// An identity hold, open or resolved, for history after a report's release
/// (REQ-RPT-002 impact): a hold resolved since still counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityHoldRecord {
    /// The hold; `candidate_subject_ids` is filled.
    pub hold: IdentityHoldView,
    /// When it was resolved; `None` while open.
    pub resolved_at: Option<UtcTimestamp>,
}

/// One version of the performing laboratory's profile (REQ-RPT-001,
/// REQ-RPT-008, REQ-BIL-005). Laboratory configuration, not PHI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaboratoryProfileView {
    /// This version's record.
    pub profile_id: LaboratoryProfileId,
    /// Version, from 1; the highest is current.
    #[ts(type = "number")]
    pub version: i64,
    /// The laboratory's name as printed.
    pub name: String,
    /// One to three address lines.
    pub address_lines: Vec<String>,
    /// CLIA certificate number (`NNDNNNNNNN`).
    pub clia_id: String,
    /// Laboratory director's name as printed.
    pub director: String,
    /// Telephone number as printed.
    pub phone: String,
    /// IANA time zone for printed and displayed local times, such as `America/Chicago`.
    pub time_zone: String,
    /// Billing NPI, when billing needs it.
    pub billing_npi: Option<String>,
    /// Tax identification number, nine digits.
    pub tax_id: Option<String>,
    /// Why this version replaced the previous one; absent on version 1.
    pub reason: Option<String>,
    /// The user who set it.
    pub set_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub set_by_display: Option<String>,
    /// When.
    pub set_at: UtcTimestamp,
}

/// A client organization with its locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientView {
    /// Whether selectable for new orders; existing records remain readable.
    pub active: bool,
    /// Current correction revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The client.
    pub id: ClientId,
    /// Lab-assigned code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Locations, by code.
    pub locations: Vec<ClientLocationView>,
}

/// A location of a client organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClientLocationView {
    /// Whether selectable for new orders; existing records remain readable.
    pub active: bool,
    /// Current correction revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The location.
    pub id: ClientLocationId,
    /// Code, unique within the client.
    pub code: String,
    /// Display name.
    pub name: String,
}

/// An ordering practitioner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PractitionerView {
    /// Whether selectable for new orders; existing records remain readable.
    pub active: bool,
    /// Current correction revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The practitioner.
    pub id: PractitionerId,
    /// The NPI (assigning authority `NPI`).
    #[ts(type = "string")]
    pub npi: Npi,
    /// Family name.
    pub family_name: String,
    /// Given names.
    pub given_names: Option<String>,
}

/// A subject as search results and other modules show it: the current
/// demographics, the active identifiers and the open hold count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectSummary {
    /// The subject.
    pub subject_id: SubjectId,
    /// Optimistic-concurrency revision for subject commands.
    #[ts(type = "number")]
    pub revision: i64,
    /// Family name of the current assertion.
    pub family_name: Phi<String>,
    /// Given names of the current assertion.
    pub given_names: Option<Phi<String>>,
    /// Birth date of the current assertion (`YYYY-MM-DD`).
    pub birth_date: Phi<String>,
    /// Administrative sex of the current assertion, as provided.
    pub administrative_sex: Option<Phi<String>>,
    /// Active identifiers, oldest first.
    pub identifiers: Vec<IdentifierView>,
    /// Open identity holds on the subject or listing it as a candidate.
    #[ts(type = "number")]
    pub open_identity_holds: usize,
}

/// [`SubjectMatches`] with each subject summarized, for review before
/// registration (REQ-IDN-001).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectCandidates {
    /// Subjects holding one of the identifiers: the same person.
    pub identifier_matches: Vec<SubjectSummary>,
    /// Subjects sharing name and birth date: to acknowledge, never linked.
    pub candidates: Vec<SubjectSummary>,
}
