//! Data contract of the `tox-report` v2 template: exactly what a clinical
//! toxicology report prints, as engine-formatted strings (REQ-RPT-001,
//! REQ-RPT-002, REQ-RPT-005, REQ-RPT-008). The template validates the
//! contract again, so JSON stored earlier is checked when it is rendered.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{DataError, RenderError};
use crate::laboratory::ReportLaboratory;
use crate::renderer::{RenderedPdf, render};
use crate::template::TemplateRef;

/// Everything a `tox-report` v2 prints. `Debug` is redacted because the
/// content is PHI.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToxReportData {
    /// Report identity, revision and status.
    pub report: ToxReportSection,
    /// The performing laboratory.
    pub laboratory: ReportLaboratory,
    /// The patient.
    pub subject: ToxSubject,
    /// The order as received.
    pub order: ToxOrder,
    /// The specimen.
    pub specimen: ToxSpecimen,
    /// Medications as supplied by the ordering source.
    pub medications: Vec<ToxMedication>,
    /// Reported results, one group per ordered component.
    pub result_groups: Vec<ToxResultGroup>,
    /// Ordered tests not reported in this revision, with their status.
    pub other_tests: Vec<ToxOtherTest>,
    /// Specimen validity outcome.
    pub validity: ToxValidity,
    /// The person who released the revision.
    pub released_by: ToxReleasedBy,
}

impl fmt::Debug for ToxReportData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToxReportData").finish_non_exhaustive()
    }
}

/// Report identity, completeness and relationship to earlier revisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxReportSection {
    /// Report number.
    pub number: String,
    /// Revision number of this release.
    pub revision: String,
    /// Whether every ordered test is resolved.
    pub completeness: ToxCompleteness,
    /// How this revision relates to the previous one.
    pub relationship: ToxRelationship,
    /// The revision replaced or supplemented; null exactly for `initial`.
    pub supersedes: Option<ToxSupersedes>,
    /// Reason for a correction; given exactly for `correction`.
    pub amendment_reason: Option<String>,
    /// Issued time, already formatted in the laboratory's time zone.
    pub issued_at: String,
    /// Explicit preparation timestamp for v3; absent on historical v2 copies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub generated_at: Option<String>,
    /// Changes in a cumulative correction (template v3 onward).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<_>", optional)]
    pub change_summary: Vec<String>,
}

/// Partial or final; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToxCompleteness {
    /// Some ordered test is pending, held or awaiting a reflex decision.
    Partial,
    /// Every ordered test is reported, cancelled or not activated.
    Final,
}

/// Relationship to the previous revision; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToxRelationship {
    /// The first revision.
    Initial,
    /// Adds results not reported before.
    Supplement,
    /// Changes reported content; needs a reason.
    Correction,
}

/// The earlier revision a supplement or correction follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxSupersedes {
    /// Revision number.
    pub revision: String,
    /// Release time, formatted in the laboratory's time zone.
    pub released_at: String,
}

/// The patient as printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxSubject {
    /// Name as printed.
    pub name: String,
    /// Birth date as printed.
    pub birth_date: String,
    /// Administrative sex as provided; null prints as unknown.
    pub sex: Option<String>,
    /// Identifiers with their assigning authority.
    pub identifiers: Vec<ToxIdentifier>,
}

/// One patient identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxIdentifier {
    /// Identifier type, for example `MRN`.
    #[serde(rename = "type")]
    pub identifier_type: String,
    /// The identifier.
    pub value: String,
    /// The assigning authority.
    pub authority: String,
}

/// The order as received from the ordering source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxOrder {
    /// The ordering practitioner.
    pub practitioner: ToxPractitioner,
    /// The ordering client.
    pub client: String,
    /// The client location, if any.
    pub location: Option<String>,
    /// The placer's order number, if any.
    pub placer_order: Option<String>,
}

/// The ordering practitioner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxPractitioner {
    /// Name as printed.
    pub name: String,
    /// National Provider Identifier.
    pub npi: String,
}

/// The specimen as printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxSpecimen {
    /// Accession number.
    pub accession: String,
    /// Specimen type.
    pub matrix: String,
    /// Container labels.
    pub container_labels: Vec<String>,
    /// Collection time as provided; null prints as unknown.
    pub collected: Option<String>,
    /// Receipt time; null prints as unknown.
    pub received: Option<String>,
    /// Specimen condition notes recorded at receipt.
    pub condition_notes: Vec<String>,
    /// Observations recorded at collection.
    pub collection_observations: Vec<ToxCollectionObservation>,
}

/// One observation recorded at collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxCollectionObservation {
    /// What was observed.
    pub code: String,
    /// The observed value as recorded.
    pub value: String,
    /// Who observed it.
    pub observed_by: String,
}

/// One medication as supplied by the ordering source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxMedication {
    /// Medication name as supplied.
    pub name: String,
    /// Where the medication list came from.
    pub source: String,
    /// When the list was current; null prints as unknown.
    pub as_of: Option<String>,
}

/// The results of one ordered component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxResultGroup {
    /// The orderable component.
    pub name: String,
    /// Method name and version.
    pub method: String,
    /// Method certainty, for example presumptive or definitive.
    pub certainty: String,
    /// The component's role, for example screening or confirmation.
    pub role: String,
    /// Approved limitations/comments from the pinned method version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub report_notes: Option<String>,
    /// One row per analyte.
    pub rows: Vec<ToxResultRow>,
}

/// One analyte result, worded by the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxResultRow {
    /// Analyte name.
    pub analyte: String,
    /// The exact lexical result or the engine's result wording.
    pub result: String,
    /// Units; empty for qualitative results.
    pub units: String,
    /// Cutoff and quantitation limits; never empty.
    pub limits: String,
    /// The engine's comparison wording, never an interpretation by the template.
    pub comparison: String,
    /// Result revision recorded time, distinct from generation/release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub recorded_at: Option<String>,
    /// Previous first release of this exact result; absent/null means this is its first report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub first_reported_at: Option<String>,
    /// Pertinent reference information, distinct from cutoff and analytical limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub reference_interval: Option<String>,
    /// Actual historical performing locations, or explicit unknown provenance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<_>", optional)]
    pub performing_sites: Vec<String>,
}

/// An ordered test not reported in this revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxOtherTest {
    /// The orderable component.
    pub name: String,
    /// Why it is not reported.
    pub status: ToxOtherTestStatus,
    /// Explanatory note; may be empty.
    pub note: String,
}

/// Why an ordered test is not reported; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToxOtherTestStatus {
    /// Results are not yet available.
    Pending,
    /// Held by a gate.
    Held,
    /// Cancelled.
    Cancelled,
    /// A reflex test whose condition was not met.
    NotActivated,
    /// A reflex test awaiting its trigger assessment.
    TriggerPending,
}

/// Specimen validity as worded by the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxValidity {
    /// Validity outcome.
    pub outcome: String,
    /// Validity notes.
    pub notes: Vec<String>,
}

/// The releaser; no title is printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToxReleasedBy {
    /// Name as printed.
    pub name: String,
}

/// The JSON [`render_json`] takes: compact, camelCase, strings or null only.
/// Store it as given so a later render uses the same bytes (NFR-07).
pub fn to_json(data: &ToxReportData) -> Result<String, DataError> {
    serde_json::to_string(data).map_err(|_| DataError::Unserializable)
}

/// Renders report JSON from [`to_json`] with `tox-report` v2. The template
/// refuses JSON that breaks the contract, for example a correction without a
/// reason or a row without limits (REQ-RPT-002, REQ-RPT-008).
pub fn render_json(
    data_json: &str,
    issued_at: jiff::Timestamp,
) -> Result<RenderedPdf, RenderError> {
    let template = TemplateRef {
        id: "tox-report".to_owned(),
        version: 2,
    };
    render(&template, data_json, issued_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::rendered_text;

    const FIXTURE: &str = include_str!("../tests/fixtures/synthetic-report-v2.json");

    fn text_of(data: &ToxReportData) -> String {
        let template = TemplateRef {
            id: "tox-report".to_owned(),
            version: 2,
        };
        rendered_text(&template, &to_json(data).expect("serializes")).expect("renders")
    }

    fn fixture() -> ToxReportData {
        serde_json::from_str(FIXTURE).expect("fixture parses")
    }

    // REQ-RPT-008: comparison wording comes from the data, never the template.
    #[test]
    fn prints_engine_wording_and_tests_not_reported() {
        let text = text_of(&fixture());
        for expected in [
            "Meetscutoff",
            "Doesnotmeetcutoff",
            "<0.50",
            "Cutoff50ng/mL;LOQ5ng/mL",
            "Testsnotreported",
            "Syntheticreflextest",
            "Notactivated",
            "CompletenessFinalreport",
            "ReporttypeInitialreport",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn issued_time_is_printed_as_given() {
        let text = text_of(&fixture());
        assert!(text.contains("Issued2026-10-1209:30CDT"));
        assert!(!text.contains("UTC"));
    }

    #[test]
    fn correction_prints_reason_and_superseded_revision() {
        let mut data = fixture();
        data.report.relationship = ToxRelationship::Correction;
        data.report.supersedes = Some(ToxSupersedes {
            revision: "1".to_owned(),
            released_at: "2026-10-11 16:00 CDT".to_owned(),
        });
        data.report.amendment_reason = Some("Synthetic correction reason".to_owned());
        let text = text_of(&data);
        assert!(text.contains("replacesrevision1,released2026-10-1116:00CDT"));
        assert!(text.contains("Syntheticcorrectionreason"));
    }

    #[test]
    fn template_has_no_positive_or_negative_wording() {
        let template = crate::template::TEMPLATES
            .iter()
            .find(|template| template.id == "tox-report" && template.version == 2)
            .expect("registered");
        let source = template.source.to_lowercase();
        assert!(!source.contains("positive") && !source.contains("negative"));
    }
}
