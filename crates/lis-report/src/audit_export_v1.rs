//! Data contract of the `audit-export` v1 template: the audit events matching
//! an export's filters, with who exported them, when, and the filters used
//! (REQ-OPS-001). Changes may hold patient values the exporter may read; the
//! template then marks every page confidential.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{DataError, RenderError};
use crate::marking::Marking;
use crate::renderer::{RenderedPdf, render_marked};
use crate::template::TemplateRef;

/// Everything an `audit-export` v1 prints. `Debug` is redacted because the
/// events may name patients.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportDocument {
    /// Laboratory name, printed in the page header.
    pub laboratory_name: String,
    /// Who exported, when, and what the file covers.
    pub export: AuditExportHeader,
    /// The filters applied, in words; empty for every event of every stream.
    pub filters: Vec<AuditExportFilter>,
    /// The events, newest first.
    pub events: Vec<AuditExportEvent>,
}

impl fmt::Debug for AuditExportDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportDocument")
            .finish_non_exhaustive()
    }
}

/// The export's provenance; times already formatted in the laboratory's time zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportHeader {
    /// When the file was exported.
    pub exported_at: String,
    /// Display name and user ID of the person who exported it.
    pub exported_by: String,
    /// The export covers events recorded up to this instant.
    pub recorded_before: String,
    /// Number of events in the file.
    pub event_count: String,
    /// `yes` when an event shows patient values or identification.
    pub patient_data: String,
}

/// One filter, such as `Stream: Clinical`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportFilter {
    /// The filter's label.
    pub name: String,
    /// Its value, in words.
    pub value: String,
}

/// One event as printed; an empty string prints as a dash.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportEvent {
    /// Stream label.
    pub stream: String,
    /// Sequence number in the stream.
    pub seq: String,
    /// When the action happened.
    pub occurred: String,
    /// When the event was recorded.
    pub recorded: String,
    /// Who acted, with their ID.
    pub by: String,
    /// Action label and code.
    pub action: String,
    /// Succeeded, Denied or Failed.
    pub outcome: String,
    /// Record type, label and ID.
    pub record: String,
    /// Reason as entered.
    pub reason: String,
    /// Capabilities that permitted the action.
    pub authority: String,
    /// `old → new` record revision.
    pub revision: String,
    /// Changed fields as `Field: old → new`, or what the Changes section says.
    pub changes: String,
    /// Other recorded details, labeled.
    pub details: String,
    /// Shared with the diagnostic log line.
    pub correlation_id: String,
}

impl fmt::Debug for AuditExportEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportEvent").finish_non_exhaustive()
    }
}

/// The JSON [`render_json`] takes: compact, camelCase, strings only.
pub fn to_json(data: &AuditExportDocument) -> Result<String, DataError> {
    serde_json::to_string(data).map_err(|_| DataError::Unserializable)
}

/// Renders export JSON from [`to_json`] with `audit-export` v1 and the
/// installation's marking; the template refuses any key outside its contract.
pub fn render_json(
    data_json: &str,
    issued_at: jiff::Timestamp,
    marking: Marking,
) -> Result<RenderedPdf, RenderError> {
    let template = TemplateRef {
        id: "audit-export".to_owned(),
        version: 1,
    };
    render_marked(&template, data_json, issued_at, marking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::rendered_text;

    #[test]
    fn prints_the_provenance_filters_events_and_confidential_marking() {
        let data: AuditExportDocument = serde_json::from_str(include_str!(
            "../tests/fixtures/synthetic-audit-export.json"
        ))
        .expect("fixture parses");
        let template = TemplateRef {
            id: "audit-export".to_owned(),
            version: 1,
        };
        let text = rendered_text(&template, &to_json(&data).expect("serializes")).expect("renders");
        assert!(text.contains("Audittrailexport"));
        assert!(text.contains("Containspatientinformation:confidential"));
        assert!(text.contains("Exportedby:TestAuditor"));
        assert!(text.contains("Stream:Clinical"));
        assert!(text.contains("Familyname:Dae→Doe"));
        assert!(text.contains("Typointhefamilyname"));
    }

    #[test]
    fn refuses_a_key_outside_the_contract() {
        let mut data: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/synthetic-audit-export.json"
        ))
        .expect("fixture parses");
        data["events"][0]["result"] = serde_json::Value::String("positive".to_owned());
        let template = TemplateRef {
            id: "audit-export".to_owned(),
            version: 1,
        };
        assert!(rendered_text(&template, &data.to_string()).is_err());
    }
}
