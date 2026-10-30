//! Read models returned by queries. Decimals stay lexical text and IDs are
//! strings when serialized.

use serde::Serialize;
use ts_rs::TS;

use crate::content::{MethodAnalyte, ValidityCategory};
use crate::lifecycle::VersionLifecycle;
use crate::model::{
    AnalyteId, ComponentPurpose, MethodCertainty, MethodId, MethodVersion, MethodVersionId,
    OrderableId, OrderableKind, OrderableVersion, OrderableVersionId, OutputScale, SpecimenTypeId,
    ThresholdRounding, ThresholdRule, ValidityPolicyVersion, ValidityPolicyVersionId,
};

/// A method version exactly as stored; for an approved or retired version,
/// exactly as approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MethodVersionView {
    /// The version.
    pub id: MethodVersionId,
    /// Its method.
    pub method_id: MethodId,
    /// The method's code.
    pub method_code: String,
    /// The method's name.
    pub method_name: String,
    /// Version number within the method.
    #[ts(type = "number")]
    pub version: i64,
    /// Lifecycle and effective period.
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle<MethodVersion>,
    /// The specimen type.
    pub specimen_type_id: SpecimenTypeId,
    /// The specimen type's code.
    pub specimen_type_code: String,
    /// Presumptive or definitive.
    pub certainty: MethodCertainty,
    /// Qualitative or quantitative.
    pub output_scale: OutputScale,
    /// Positive at ≥ or at > the cutoff.
    pub threshold_rule: ThresholdRule,
    /// Comparison before or after rounding.
    pub threshold_rounding: ThresholdRounding,
    /// Laboratory-approved method information and limitations for reports.
    pub report_notes: Option<String>,
    /// Optional external approval record, excluded from patient-report content.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub external_approval_reference: Option<String>,
    /// Analytes by code, with their limits.
    pub analytes: Vec<MethodAnalyteView>,
}

/// One analyte of a method version with its limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MethodAnalyteView {
    /// The analyte's code.
    pub analyte_code: String,
    /// The analyte's name.
    pub analyte_name: String,
    /// The limits as entered.
    #[serde(flatten)]
    pub limits: MethodAnalyte,
}

/// An orderable version with its component manifest. Orders freeze the
/// version ID; reading it later returns exactly this manifest (REQ-ORD-002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderableVersionView {
    /// The version.
    pub id: OrderableVersionId,
    /// Its orderable.
    pub orderable_id: OrderableId,
    /// The orderable's code.
    pub code: String,
    /// The orderable's name.
    pub name: String,
    /// Test or panel.
    pub kind: OrderableKind,
    /// Version number within the orderable.
    #[ts(type = "number")]
    pub version: i64,
    /// Lifecycle and effective period.
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle<OrderableVersion>,
    /// Components by position.
    pub components: Vec<ComponentView>,
}

/// One component of an orderable version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "CatalogComponentView")]
pub struct ComponentView {
    /// Position within the version, from 1; with the version ID it
    /// identifies the component.
    #[ts(type = "number")]
    pub position: i64,
    /// Analytical or validity.
    pub purpose: ComponentPurpose,
    /// The method version the component uses.
    pub method_version_id: MethodVersionId,
    /// The method's code.
    pub method_code: String,
    /// The method version's number.
    #[ts(type = "number")]
    pub method_version: i64,
    /// The method version's specimen type code.
    pub specimen_type_code: String,
    /// Presumptive or definitive.
    pub certainty: MethodCertainty,
    /// Qualitative or quantitative.
    pub output_scale: OutputScale,
    /// The analytes the component expects, by code.
    pub analytes: Vec<ComponentAnalyteView>,
}

/// An analyte a component expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "CatalogComponentAnalyteView")]
pub struct ComponentAnalyteView {
    /// The analyte.
    pub analyte_id: AnalyteId,
    /// Its code.
    pub code: String,
    /// Its name.
    pub name: String,
    /// Its LOINC code as provided.
    pub loinc_code: Option<String>,
    /// Its drug class code as provided.
    pub drug_class_code: Option<String>,
}

/// A validity policy version with its categories (REQ-SPC-008); for an
/// approved or retired version, exactly as approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ValidityPolicyView {
    /// The version.
    pub id: ValidityPolicyVersionId,
    /// The specimen type the policy applies to.
    pub specimen_type_id: SpecimenTypeId,
    /// The specimen type's code.
    pub specimen_type_code: String,
    /// Version number within the specimen type.
    #[ts(type = "number")]
    pub version: i64,
    /// Lifecycle and effective period.
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle<ValidityPolicyVersion>,
    /// Categories by code.
    pub categories: Vec<ValidityCategory>,
}

/// A specimen type, with the state of its validity policy versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpecimenTypeView {
    /// The specimen type.
    pub id: SpecimenTypeId,
    /// Lab-defined code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Number of the validity policy version in effect (approved, effective
    /// period open), if any.
    #[ts(type = "number | null")]
    pub version_in_effect: Option<i64>,
    /// Number of the validity policy draft awaiting approval, if any.
    #[ts(type = "number | null")]
    pub draft_version: Option<i64>,
}

/// An analyte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalyteView {
    /// The analyte.
    pub id: AnalyteId,
    /// Lab-defined code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// LOINC code as provided.
    pub loinc_code: Option<String>,
    /// Drug class code as provided.
    pub drug_class_code: Option<String>,
    /// Offered for new method versions and source test-code maps; a retired
    /// analyte stays in existing records.
    pub active: bool,
    /// Current revision; each correction or retirement adds one.
    #[ts(type = "number")]
    pub revision: i64,
}

/// A method; its content lives in its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MethodView {
    /// The method.
    pub id: MethodId,
    /// Lab-defined code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Number of the version in effect (approved, effective period open), if
    /// any.
    #[ts(type = "number | null")]
    pub version_in_effect: Option<i64>,
    /// Number of the draft awaiting approval, if any.
    #[ts(type = "number | null")]
    pub draft_version: Option<i64>,
}

/// A test or panel; its manifest lives in its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderableView {
    /// The orderable.
    pub id: OrderableId,
    /// Lab-defined code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Test or panel.
    pub kind: OrderableKind,
    /// Number of the version in effect (approved, effective period open), if
    /// any.
    #[ts(type = "number | null")]
    pub version_in_effect: Option<i64>,
    /// Number of the draft awaiting approval, if any.
    #[ts(type = "number | null")]
    pub draft_version: Option<i64>,
}
