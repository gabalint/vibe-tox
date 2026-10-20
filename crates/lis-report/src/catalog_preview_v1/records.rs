//! Method, test and panel, and validity records of the Catalog Preview, and the
//! source labels their values carry.

use serde::{Deserialize, Serialize};

/// Where a printed value came from; serialized with a snake case `kind`.
/// The template prints the label wording, so the engine supplies only the kind
/// and, for published values, the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SourceLabel {
    /// Stated in a published source.
    Published {
        /// Title of the source.
        title: String,
        /// The source's short label for the value, for example "Federal
        /// workplace initial test cutoff".
        label: String,
    },
    /// A starter value the laboratory must confirm.
    StarterValue,
    /// No source gives the value; the laboratory must supply it.
    Blank,
    /// A fictitious value for the demo laboratory.
    DemoValue,
    /// The laboratory changed a supplied value.
    ChangedByLaboratory,
    /// The laboratory entered the value itself.
    EnteredByLaboratory,
}

/// Whether a version is a draft or in effect; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewVersionStatus {
    /// Awaiting review.
    Draft,
    /// Approved and current.
    InEffect,
}

/// A method with its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewMethod {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
    /// Draft and in-effect versions.
    pub versions: Vec<PreviewMethodVersion>,
}

/// Method certainty; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewCertainty {
    /// Presumptive (screening).
    Presumptive,
    /// Definitive (confirmation).
    Definitive,
}

/// Method output scale; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewOutputScale {
    /// Qualitative results.
    Qualitative,
    /// Quantitative results.
    Quantitative,
}

/// How a result is compared with the cutoff; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewThresholdRule {
    /// A result at the cutoff meets it.
    AtOrAbove,
    /// A result must exceed the cutoff.
    Above,
}

/// When rounding applies relative to the comparison; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewThresholdRounding {
    /// Compare before rounding.
    BeforeRounding,
    /// Compare after rounding.
    AfterRounding,
}

/// A threshold rule with its source label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcedThresholdRule {
    /// The rule.
    pub value: PreviewThresholdRule,
    /// Where it came from.
    pub source: SourceLabel,
}

/// A threshold rounding with its source label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcedThresholdRounding {
    /// The rounding.
    pub value: PreviewThresholdRounding,
    /// Where it came from.
    pub source: SourceLabel,
}

/// One version of a method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewMethodVersion {
    /// Version number.
    pub version: String,
    /// Draft or in effect.
    pub status: PreviewVersionStatus,
    /// Specimen type, as printed.
    pub specimen_type: String,
    /// Presumptive or definitive.
    pub certainty: PreviewCertainty,
    /// Qualitative or quantitative.
    pub output_scale: PreviewOutputScale,
    /// Threshold rule with its source.
    pub threshold_rule: SourcedThresholdRule,
    /// Threshold rounding with its source.
    pub threshold_rounding: SourcedThresholdRounding,
    /// One row per analyte.
    pub analytes: Vec<PreviewMethodAnalyte>,
}

/// An analyte's unit, cutoff and limits in one method version. Null means
/// blank. A cutoff is null exactly when its source is [`SourceLabel::Blank`];
/// the template refuses anything else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewMethodAnalyte {
    /// Analyte code.
    pub code: String,
    /// Analyte name.
    pub name: String,
    /// Unit, if any.
    pub unit: Option<String>,
    /// Cutoff as a lexical decimal.
    pub cutoff: Option<String>,
    /// Where the cutoff came from.
    pub cutoff_source: SourceLabel,
    /// Limit of detection.
    pub lod: Option<String>,
    /// Limit of quantitation.
    pub loq: Option<String>,
    /// Lower end of the reportable range.
    pub reportable_low: Option<String>,
    /// Upper end of the reportable range.
    pub reportable_high: Option<String>,
    /// Display scale (decimal places).
    pub display_scale: Option<String>,
}

/// A test or panel with its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOrderable {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
    /// Test or panel.
    pub kind: PreviewOrderableKind,
    /// Draft and in-effect versions.
    pub versions: Vec<PreviewOrderableVersion>,
}

/// Test or panel; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewOrderableKind {
    /// A single-component test.
    Test,
    /// A panel of components.
    Panel,
}

/// One version of a test or panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOrderableVersion {
    /// Version number.
    pub version: String,
    /// Draft or in effect.
    pub status: PreviewVersionStatus,
    /// Components in position order.
    pub components: Vec<PreviewComponent>,
}

/// Whether a component measures analytes or specimen validity; serialized in
/// snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewComponentPurpose {
    /// Analytical component.
    Analytical,
    /// Specimen validity component.
    Validity,
}

/// One component of a test or panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewComponent {
    /// Position within the test or panel.
    pub position: String,
    /// Code of the method used.
    pub method_code: String,
    /// Version of the method used.
    pub method_version: String,
    /// Analytical or validity.
    pub purpose: PreviewComponentPurpose,
    /// Analytes the component reports.
    pub analyte_codes: Vec<String>,
}

/// The validity policy of one specimen type, with its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewValidityPolicy {
    /// Specimen type, as printed.
    pub specimen_type: String,
    /// Draft and in-effect versions.
    pub versions: Vec<PreviewValidityVersion>,
}

/// One version of a validity policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewValidityVersion {
    /// Version number.
    pub version: String,
    /// Draft or in effect.
    pub status: PreviewVersionStatus,
    /// Validity categories.
    pub categories: Vec<PreviewValidityCategory>,
}

/// What a validity category does; serialized in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewValidityAction {
    /// No action.
    None,
    /// A qualifier is added to the report.
    Qualifier,
    /// Release is held.
    Hold,
    /// Recollection is requested.
    Recollect,
    /// A comment is added.
    Comment,
}

/// One validity category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewValidityCategory {
    /// Code.
    pub code: String,
    /// Label.
    pub label: String,
    /// Action.
    pub action: PreviewValidityAction,
    /// Qualifier text, when the action uses one.
    pub qualifier_text: Option<String>,
}
