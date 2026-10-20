//! Data contract of the `report-notice` v1 template: a withdrawal notice that
//! identifies the report and carries a fixed statement, with no
//! demographics, results or reason (REQ-RPT-002).

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{DataError, RenderError};
use crate::laboratory::ReportLaboratory;
use crate::renderer::{RenderedPdf, render};
use crate::template::TemplateRef;

/// Everything a `report-notice` v1 prints. `Debug` is redacted because the
/// accession links the notice to a patient.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportNoticeData {
    /// The laboratory issuing the notice.
    pub laboratory: ReportLaboratory,
    /// The report the notice concerns.
    pub notice: ReportNoticeSection,
}

impl fmt::Debug for ReportNoticeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportNoticeData").finish_non_exhaustive()
    }
}

/// Identifies the withdrawn report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReportNoticeSection {
    /// Report number.
    pub report_number: String,
    /// The report's latest revision when the notice was issued.
    pub revision: String,
    /// Accession number of the specimen.
    pub accession: String,
    /// What the notice announces.
    pub kind: ReportNoticeKind,
    /// Issued time, already formatted in the laboratory's time zone.
    pub issued_at: String,
    /// Name of the person who issued the notice.
    pub issued_by: String,
}

/// What a notice announces; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReportNoticeKind {
    /// The laboratory withdrew the report; no revision may be used.
    Withdrawal,
}

/// The JSON [`render_json`] takes: compact, camelCase, strings only. Store it
/// as given so a later render uses the same bytes (NFR-07).
pub fn to_json(data: &ReportNoticeData) -> Result<String, DataError> {
    serde_json::to_string(data).map_err(|_| DataError::Unserializable)
}

/// Renders notice JSON from [`to_json`] with `report-notice` v1; the template
/// refuses unknown kinds and any key outside the contract.
pub fn render_json(
    data_json: &str,
    issued_at: jiff::Timestamp,
) -> Result<RenderedPdf, RenderError> {
    let template = TemplateRef {
        id: "report-notice".to_owned(),
        version: 1,
    };
    render(&template, data_json, issued_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::rendered_text;

    #[test]
    fn prints_the_fixed_withdrawal_statement() {
        let data: ReportNoticeData =
            serde_json::from_str(include_str!("../tests/fixtures/synthetic-notice.json"))
                .expect("fixture parses");
        let template = TemplateRef {
            id: "report-notice".to_owned(),
            version: 1,
        };
        let text = rendered_text(&template, &to_json(&data).expect("serializes")).expect("renders");
        assert!(text.contains("ThelaboratoryhaswithdrawnreportRPT-SYN-0002."));
        assert!(text.contains("Revision2andeveryearlierrevisionofthisreportmustnotbeused."));
        assert!(text.contains("Issued2026-10-1310:05CDT"));
    }
}
