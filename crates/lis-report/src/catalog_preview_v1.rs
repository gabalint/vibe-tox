//! Version 1 data contract used by `catalog-preview` templates v1 and v2: a listing of a
//! laboratory's whole test catalog, so the laboratory manager can check every
//! value on paper before approving it (proposed REQ-OPS-015).
//!
//! The types are independent of `lis-catalog`; the engine maps catalog data
//! into [`CatalogPreviewDocument`]. Every numeric value is a lexical decimal
//! string, printed exactly as given. The template validates the contract
//! again, so JSON stored earlier is checked when it is rendered.

use serde::{Deserialize, Serialize};

use crate::error::{DataError, RenderError};
use crate::marking::Marking;
use crate::renderer::{RenderedPdf, render_marked};
use crate::template::TemplateRef;

mod records;
pub use records::{
    PreviewCertainty, PreviewComponent, PreviewComponentPurpose, PreviewMethod,
    PreviewMethodAnalyte, PreviewMethodVersion, PreviewOrderable, PreviewOrderableKind,
    PreviewOrderableVersion, PreviewOutputScale, PreviewThresholdRounding, PreviewThresholdRule,
    PreviewValidityAction, PreviewValidityCategory, PreviewValidityPolicy, PreviewValidityVersion,
    PreviewVersionStatus, SourceLabel, SourcedThresholdRounding, SourcedThresholdRule,
};

/// Everything a `catalog-preview` prints, in print order after the header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewDocument {
    /// Report identity and provenance.
    pub report: CatalogPreviewHeader,
    /// Laboratory name as printed in the page header.
    pub laboratory_name: String,
    /// Shown prominently on page 1 when present, for example "Starter
    /// content: check every value before use. It is not validated for your
    /// laboratory and not ready for patient testing."
    pub disclaimer: Option<String>,
    /// Blank cutoffs and starter values to confirm.
    pub gaps: Vec<CatalogGap>,
    /// Drafts this report covers.
    pub drafts: Vec<CatalogDraft>,
    /// Specimen types.
    pub specimen_types: Vec<PreviewSpecimenType>,
    /// Analytes.
    pub analytes: Vec<PreviewAnalyte>,
    /// Methods with their draft and in-effect versions.
    pub methods: Vec<PreviewMethod>,
    /// Tests and panels with their draft and in-effect versions.
    pub orderables: Vec<PreviewOrderable>,
    /// Specimen validity policies with their versions.
    pub validity_policies: Vec<PreviewValidityPolicy>,
    /// Regenstrief's LOINC notice, printed at the end when present.
    pub loinc_notice: Option<String>,
}

/// Report identity, generation time and who generated it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewHeader {
    /// Report identifier, printed on every page.
    pub id: String,
    /// Generation time, already formatted in the laboratory's time zone.
    pub generated_at: String,
    /// The same instant in UTC, already formatted.
    pub generated_at_utc: String,
    /// Display name of the user who generated the report.
    pub generated_by: String,
    /// Digest of the report's content, as computed by the engine.
    pub content_digest: String,
}

/// What a gap is; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// An analyte's cutoff is blank.
    BlankCutoff,
    /// A method's threshold rule is a starter value to confirm.
    StarterThresholdRule,
    /// A method's threshold rounding is a starter value to confirm.
    StarterThresholdRounding,
}

/// One value the laboratory must supply or confirm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGap {
    /// The record, for example a method code with its version.
    pub record: String,
    /// The analyte code; null for gaps that concern the whole method.
    pub analyte: Option<String>,
    /// What is missing.
    pub gap: GapKind,
}

/// The kind of record a draft is; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftRecordType {
    /// A specimen validity policy version.
    ValidityPolicy,
    /// A method version.
    Method,
    /// A test version.
    Test,
    /// A panel version.
    Panel,
}

/// One draft awaiting review, with the revision the report covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDraft {
    /// Kind of record.
    pub record_type: DraftRecordType,
    /// Record code.
    pub code: String,
    /// Record name.
    pub name: String,
    /// Version number.
    pub version: String,
    /// Revision of the draft when the report was generated.
    pub revision: String,
}

/// A specimen type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSpecimenType {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
}

/// An analyte with its LOINC mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAnalyte {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
    /// LOINC code, if mapped.
    pub loinc_code: Option<String>,
    /// LOINC display name, if mapped.
    pub loinc_display_name: Option<String>,
    /// Drug class, if any.
    pub drug_class: Option<String>,
}

/// The JSON [`render_json`] takes: compact, camelCase, strings or null only.
/// Preserve it with the template reference for later reproduction (NFR-07).
pub fn to_json(data: &CatalogPreviewDocument) -> Result<String, DataError> {
    serde_json::to_string(data).map_err(|_| DataError::Unserializable)
}

/// Renders document JSON from [`to_json`] with the current `catalog-preview` v2 template. The
/// template refuses unknown codes, empty required values and a cutoff that
/// disagrees with its source label. `issued_at` is the generation instant; it
/// sets the PDF date and nothing else. `marking` is the installation's: the demo
/// laboratory's preview is watermarked (proposed REQ-OPS-016); an unmarked one is
/// the template's output, byte for byte. Reproduce an older template with
/// [`crate::render_marked`] and its recorded [`TemplateRef`].
pub fn render_json(
    data_json: &str,
    issued_at: jiff::Timestamp,
    marking: Marking,
) -> Result<RenderedPdf, RenderError> {
    let template = TemplateRef {
        id: "catalog-preview".to_owned(),
        version: 2,
    };
    render_marked(&template, data_json, issued_at, marking)
}

#[cfg(test)]
mod tests;
