//! Where each printed value came from (REQ-OPS-015): matched by code against
//! the Standard Catalog file, so no schema change is needed.

use std::collections::BTreeMap;

use lis_core::{DecimalText, text_enum};
use serde::Serialize;

use crate::model::{AnalyteId, MethodVersionId, ThresholdRounding, ThresholdRule, VersionStatus};
use crate::standard::format::{Method, StandardCatalog};

text_enum! {
    /// The source of a printed catalog value.
    CatalogValueSourceKind {
        /// Equals the value the file publishes, with its source.
        Published = "published",
        /// Equals the demo laboratory's value; not a recommended cutoff.
        DemoValue = "demo_value",
        /// No value; the laboratory supplies it or confirms it as not applicable.
        Blank = "blank",
        /// A saved value differing from the current file; its original source is not inferred.
        ChangedByLaboratory = "changed_by_laboratory",
        /// An item the laboratory created itself.
        LaboratoryEntered = "laboratory_entered",
        /// A required field still holding the file's starter value; confirm it.
        StarterValue = "starter_value",
    }
}

text_enum! {
    /// Which field of a method version a gap concerns.
    CatalogGapField {
        /// An analyte's cutoff.
        Cutoff = "cutoff",
        /// The threshold rule.
        ThresholdRule = "threshold_rule",
        /// The threshold rounding.
        ThresholdRounding = "threshold_rounding",
    }
}

/// A value's source, with the publication for published values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogValueSource {
    /// Where the value came from.
    pub kind: CatalogValueSourceKind,
    /// The publication's title, for published values.
    pub source_title: Option<String>,
    /// The file's short label, for published values.
    pub label: Option<String>,
}

impl CatalogValueSource {
    fn of(kind: CatalogValueSourceKind) -> Self {
        Self {
            kind,
            source_title: None,
            label: None,
        }
    }
}

/// A blank cutoff or a starter value the report highlights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGap {
    /// `blank` or `starter_value`.
    pub kind: CatalogValueSourceKind,
    /// The field.
    pub field: CatalogGapField,
    /// The method version.
    pub method_version_id: MethodVersionId,
    /// The method's code.
    pub method_code: String,
    /// The version number.
    pub version: i64,
    /// Draft or approved.
    pub status: VersionStatus,
    /// The analyte, for a cutoff.
    pub analyte_id: Option<AnalyteId>,
    /// The analyte's code, for a cutoff.
    pub analyte_code: Option<String>,
}

/// Demo values by (method code, analyte code); empty in a real laboratory.
pub type DemoValues = BTreeMap<(String, String), DecimalText>;

/// The source of a method analyte's cutoff. Published and demo values are
/// matched by their exact text, which the loader copies unchanged.
pub(super) fn cutoff_source(
    c: &StandardCatalog,
    demo_values: &DemoValues,
    method_code: &str,
    analyte_code: &str,
    cutoff: Option<&DecimalText>,
) -> CatalogValueSource {
    let Some(value) = cutoff else {
        return CatalogValueSource::of(CatalogValueSourceKind::Blank);
    };
    let entry = c.method(method_code).and_then(|m| m.analyte(analyte_code));
    if let Some(published) = entry.and_then(|a| a.cutoff.as_ref())
        && published.value == *value
    {
        return CatalogValueSource {
            kind: CatalogValueSourceKind::Published,
            source_title: c.source(&published.source).map(|s| s.title.clone()),
            label: Some(published.label.clone()),
        };
    }
    let key = (method_code.to_owned(), analyte_code.to_owned());
    if demo_values.get(&key) == Some(value) {
        return CatalogValueSource::of(CatalogValueSourceKind::DemoValue);
    }
    CatalogValueSource::of(if entry.is_some() {
        CatalogValueSourceKind::ChangedByLaboratory
    } else {
        CatalogValueSourceKind::LaboratoryEntered
    })
}

/// The sources of a version's threshold rule and rounding: starter values
/// while they equal the file's method entry.
pub(super) fn threshold_sources(
    method: Option<&Method>,
    rule: ThresholdRule,
    rounding: ThresholdRounding,
) -> (CatalogValueSource, CatalogValueSource) {
    let source = |starter: Option<bool>| {
        CatalogValueSource::of(match starter {
            Some(true) => CatalogValueSourceKind::StarterValue,
            Some(false) => CatalogValueSourceKind::ChangedByLaboratory,
            None => CatalogValueSourceKind::LaboratoryEntered,
        })
    };
    (
        source(method.map(|m| m.threshold_rule == rule)),
        source(method.map(|m| m.threshold_rounding == rounding)),
    )
}
