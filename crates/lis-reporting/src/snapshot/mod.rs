//! The report snapshot: what a revision freezes (brief §3.2, REQ-RPT-001).
//! Its canonical JSON is the candidate's `content_json`; its SHA-256 is the
//! content digest that release rebuilds and compares (NFR-01). Signatory and
//! issue time live on the candidate row, and review attribution in the
//! candidate lines, so a re-review that prints nothing new is not new content.

mod build;
mod provenance;
mod sections;

use std::fmt;

use lis_catalog::{
    AnalyteId, ComponentPurpose, MethodCertainty, OrderableId, OutputScale, ValidityAction,
    ValidityPolicyVersionId,
};
use lis_core::{Comparator, DecimalText, UtcTimestamp};
use lis_identity::{LaboratoryProfileId, SubjectId};
use lis_orders::{MedicationSource, OrderComponentId, OrderId, OrderItemId, SequenceRole};
use lis_results::{ResultId, ResultOutcome, ResultRevisionId, ValidityAssessmentId};
use lis_specimens::{
    AccessionId, CollectionPrecision, ReceiptDecisionId, ReceiptDiscrepancyCode,
    SpecimenAcceptance, SpecimenId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ReportingError;
use crate::model::LineDisposition;

pub use build::{Built, build};

/// Everything a revision freezes. `Debug` is redacted: this is PHI.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportSnapshot {
    /// Specimen and collection context.
    pub specimen: SpecimenSection,
    /// The subject as identified now.
    pub subject: SubjectSection,
    /// The order as received.
    pub order: OrderSection,
    /// One line per expected component, in order and position order.
    pub lines: Vec<ComponentLine>,
    /// Reflex orderables in scope that were not activated, or await a decision.
    pub triggers: Vec<TriggerLine>,
    /// The current specimen-validity assessment, if any.
    pub validity: Option<ValiditySection>,
    /// The performing laboratory.
    pub laboratory: LaboratorySection,
}

impl fmt::Debug for ReportSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportSnapshot").finish_non_exhaustive()
    }
}

impl ReportSnapshot {
    /// The canonical JSON: struct field order, sorted lists, no maps.
    pub fn to_canonical_json(&self) -> Result<String, ReportingError> {
        Ok(serde_json::to_string(self)?)
    }

    /// The content a released revision froze, verified against its digests.
    pub(crate) fn of_revision(
        conn: &rusqlite::Connection,
        revision: &crate::repo::reports::RevisionRow,
    ) -> Result<Self, ReportingError> {
        let candidate = crate::repo::candidates::candidate(conn, revision.candidate_id)?
            .ok_or(ReportingError::Inconsistent("released candidate"))?;
        Ok(crate::stored::stored_content(conn, &candidate)?.snapshot)
    }

    /// The reported lines' analytes with their exact result revisions.
    pub(crate) fn reported(&self) -> impl Iterator<Item = (&ComponentLine, &AnalyteLine)> {
        self.lines
            .iter()
            .filter(|line| line.disposition == LineDisposition::Reported)
            .flat_map(|line| line.analytes.iter().map(move |analyte| (line, analyte)))
    }
}

/// Lowercase hex SHA-256 of `text`.
#[must_use]
pub fn sha256_hex(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// Specimen, accession and collection context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecimenSection {
    pub specimen_id: SpecimenId,
    pub specimen_type_code: String,
    pub specimen_type_name: String,
    pub accession_id: AccessionId,
    pub accession_number: String,
    pub order_item_ids: Vec<OrderItemId>,
    pub acceptance: SpecimenAcceptance,
    pub latest_decision_id: Option<ReceiptDecisionId>,
    pub conditional_policy: Option<String>,
    pub conditional_scope: Option<String>,
    pub discrepancy_codes: Vec<ReceiptDiscrepancyCode>,
    /// Receipt discrepancies only; excludes internal hold reasons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_history: Vec<ConditionSection>,
    /// Explicit reportability decisions; absent only in older frozen snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_report_review: Option<crate::ConditionReportReview>,
    pub collection_precision: CollectionPrecision,
    pub collected_at: Option<UtcTimestamp>,
    pub utc_offset_minutes: Option<i32>,
    pub collected_date: Option<String>,
    pub collector: Option<String>,
    pub observations: Vec<ObservationSection>,
    pub received_at: UtcTimestamp,
    pub container_labels: Vec<String>,
}

impl SpecimenSection {
    /// L2: the accession number, `-`, the specimen type code; PHI-free.
    #[must_use]
    pub fn report_number(&self) -> String {
        format!("{}-{}", self.accession_number, self.specimen_type_code)
    }
}

/// Captured specimen-condition evidence, including earlier held receipt decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConditionSection {
    pub decision_id: ReceiptDecisionId,
    pub decision: lis_specimens::ReceiptDecision,
    pub recorded_at: UtcTimestamp,
    pub code: ReceiptDiscrepancyCode,
    pub description: Option<String>,
}

/// One collection-site observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationSection {
    pub code: String,
    pub value: String,
    pub observed_by: String,
}

/// The subject's current identity.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubjectSection {
    pub subject_id: SubjectId,
    pub revision: i64,
    pub family_name: String,
    pub given_names: Option<String>,
    pub birth_date: String,
    pub sex: Option<String>,
    pub identifiers: Vec<IdentifierSection>,
}

impl fmt::Debug for SubjectSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubjectSection")
            .field("subject_id", &self.subject_id)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

/// One active identifier with its assigning authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentifierSection {
    pub identifier_type: String,
    pub value: String,
    pub authority: String,
}

/// The order as received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrderSection {
    pub order_id: OrderId,
    /// The evidence version the medications come from.
    pub version: i64,
    pub placer_order_id: Option<String>,
    pub practitioner_name: String,
    pub practitioner_npi: String,
    pub client_name: String,
    pub location_name: Option<String>,
    pub medications: Vec<MedicationSection>,
}

/// One current medication statement, as supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MedicationSection {
    pub name: String,
    pub source: MedicationSource,
    pub as_of: Option<String>,
}

/// One expected component and its disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComponentLine {
    pub component_id: OrderComponentId,
    pub item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub orderable_name: String,
    pub method_name: String,
    pub method_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_notes: Option<String>,
    pub certainty: MethodCertainty,
    pub output_scale: OutputScale,
    pub sequence_role: SequenceRole,
    pub purpose: ComponentPurpose,
    pub disposition: LineDisposition,
    pub analytes: Vec<AnalyteLine>,
}

/// One expected analyte, its limits and its current result, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyteLine {
    pub analyte_id: AnalyteId,
    pub name: String,
    pub unit: Option<String>,
    pub cutoff: Option<DecimalText>,
    pub limit_of_detection: Option<DecimalText>,
    pub limit_of_quantitation: Option<DecimalText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_interval: Option<String>,
    pub result: Option<ResultSection>,
}

/// The exact current result revision of an analyte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultSection {
    pub result_id: ResultId,
    pub revision_id: ResultRevisionId,
    pub revision_no: i64,
    pub outcome: ResultOutcome,
    pub comparator: Option<Comparator>,
    pub magnitude: Option<DecimalText>,
    pub units_text: Option<String>,
    pub code_text: Option<String>,
    pub classification: String,
    /// Original result revision creation time, not inferred testing completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<UtcTimestamp>,
    /// Earliest committed release containing this exact result revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_reported_at: Option<UtcTimestamp>,
    /// Actual run-site profiles, not the current reporting contact profile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performing_sites: Vec<LaboratorySection>,
    #[serde(default)]
    pub performing_site_unknown: bool,
}

/// A reflex orderable in a parent item's scope that has no activated item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerLine {
    pub parent_item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub orderable_name: String,
    pub disposition: LineDisposition,
}

/// The current validity assessment and its policy category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValiditySection {
    pub assessment_id: ValidityAssessmentId,
    pub policy_version_id: ValidityPolicyVersionId,
    pub category_code: String,
    pub label: String,
    pub action: ValidityAction,
    pub qualifier_text: Option<String>,
}

/// The laboratory profile version in force.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaboratorySection {
    pub profile_id: LaboratoryProfileId,
    pub version: i64,
    pub name: String,
    pub address_lines: Vec<String>,
    pub clia_id: String,
    pub director: String,
    pub phone: String,
    pub time_zone: String,
}
