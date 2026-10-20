//! Serde types of the Standard Catalog file, format version 1. The contract
//! is `standard/README.md`; unknown fields are refused.

use lis_core::DecimalText;
use serde::Deserialize;

use crate::model::{
    ComponentPurpose, MethodCertainty, OrderableKind, OutputScale, ThresholdRounding, ThresholdRule,
};
use crate::standard::StandardReviewStatus;

/// The whole file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StandardCatalog {
    pub(crate) format: String,
    pub(crate) format_version: u32,
    pub(crate) catalog_version: String,
    pub(crate) review_status: StandardReviewStatus,
    pub(crate) disclaimer: String,
    pub(crate) loinc: LoincRelease,
    pub(crate) sources: Vec<Source>,
    pub(crate) drug_classes: Vec<DrugClass>,
    pub(crate) specimen_types: Vec<SpecimenType>,
    pub(crate) analytes: Vec<Analyte>,
    pub(crate) methods: Vec<Method>,
    pub(crate) orderables: Vec<Orderable>,
}

/// The pinned LOINC release and Regenstrief's notice, verbatim.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LoincRelease {
    pub(crate) version: String,
    pub(crate) notice: String,
}

/// A cited publication.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Source {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) publisher: String,
    pub(crate) url: String,
    pub(crate) published: Option<String>,
    pub(crate) checked: String,
    pub(crate) note: String,
}

/// The project's own pharmacological grouping, not CPT's.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DrugClass {
    pub(crate) code: String,
    pub(crate) name: String,
}

/// A specimen type (matrix).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SpecimenType {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) sources: Vec<String>,
}

/// An analyte, specific to one specimen type.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Analyte {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) drug_class: Option<String>,
    pub(crate) loinc: Option<Loinc>,
    pub(crate) sources: Vec<String>,
}

/// A LOINC code with its display name exactly as in the pinned release.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Loinc {
    pub(crate) code: String,
    pub(crate) display_name: String,
}

/// A method; loaded as a draft method version.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Method {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) specimen_type: String,
    pub(crate) certainty: MethodCertainty,
    pub(crate) output_scale: OutputScale,
    /// A starter value the laboratory must confirm.
    pub(crate) threshold_rule: ThresholdRule,
    /// A starter value the laboratory must confirm.
    pub(crate) threshold_rounding: ThresholdRounding,
    pub(crate) analytes: Vec<MethodAnalyte>,
    pub(crate) sources: Vec<String>,
}

impl Method {
    /// The entry for the analyte with `code`, if the method measures it.
    pub(crate) fn analyte(&self, code: &str) -> Option<&MethodAnalyte> {
        self.analytes.iter().find(|a| a.analyte == code)
    }
}

/// One analyte of a method. Limits other than the cutoff are not part of the
/// format: they belong to each laboratory's validated assay.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct MethodAnalyte {
    pub(crate) analyte: String,
    pub(crate) unit: Option<String>,
    pub(crate) cutoff: Option<Cutoff>,
}

/// A published cutoff with its source; never an invented value.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Cutoff {
    pub(crate) value: DecimalText,
    pub(crate) source: String,
    pub(crate) label: String,
}

/// A test or panel.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Orderable {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) kind: OrderableKind,
    pub(crate) components: Vec<Component>,
    pub(crate) sources: Vec<String>,
}

impl Orderable {
    /// The distinct method codes of the components, in component order.
    pub(crate) fn method_codes(&self) -> Vec<&str> {
        let mut codes: Vec<&str> = Vec::new();
        for component in &self.components {
            if !codes.contains(&component.method.as_str()) {
                codes.push(&component.method);
            }
        }
        codes
    }
}

/// One component of an orderable.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Component {
    pub(crate) method: String,
    pub(crate) purpose: ComponentPurpose,
    pub(crate) analytes: Vec<String>,
}

impl StandardCatalog {
    pub(crate) fn source(&self, id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    pub(crate) fn drug_class(&self, code: &str) -> Option<&DrugClass> {
        self.drug_classes.iter().find(|c| c.code == code)
    }

    pub(crate) fn analyte(&self, code: &str) -> Option<&Analyte> {
        self.analytes.iter().find(|a| a.code == code)
    }

    pub(crate) fn method(&self, code: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.code == code)
    }
}
