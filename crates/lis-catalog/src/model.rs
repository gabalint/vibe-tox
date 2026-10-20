//! Identifiers, coded enums and text validation. Pure: no database.

use lis_core::Id;
use lis_core::text_enum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CatalogError, invalid};

/// Marker for [`SpecimenTypeId`].
pub enum SpecimenType {}
/// A lab-defined specimen type (matrix).
pub type SpecimenTypeId = Id<SpecimenType>;
/// Marker for [`AnalyteId`].
pub enum Analyte {}
/// A lab-defined analyte.
pub type AnalyteId = Id<Analyte>;
/// Marker for [`MethodId`].
pub enum Method {}
/// A method; its content lives in method versions.
pub type MethodId = Id<Method>;
/// Marker for [`MethodVersionId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodVersion {}
/// One version of a method's content.
pub type MethodVersionId = Id<MethodVersion>;
/// Marker for [`OrderableId`].
pub enum Orderable {}
/// A test or panel; its manifest lives in orderable versions.
pub type OrderableId = Id<Orderable>;
/// Marker for [`OrderableVersionId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderableVersion {}
/// One version of an orderable's component manifest.
pub type OrderableVersionId = Id<OrderableVersion>;
/// Marker for [`ValidityPolicyVersionId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidityPolicyVersion {}
/// One version of a specimen type's validity policy (REQ-SPC-008).
pub type ValidityPolicyVersionId = Id<ValidityPolicyVersion>;
/// Marker for [`StandardCatalogLoadId`].
pub enum StandardCatalogLoad {}
/// One load of Standard Catalog content (REQ-OPS-014).
pub type StandardCatalogLoadId = Id<StandardCatalogLoad>;
/// Marker for [`CatalogPreviewId`].
pub enum CatalogPreview {}
/// One generated Catalog Preview report (REQ-OPS-015).
pub type CatalogPreviewId = Id<CatalogPreview>;
/// Marker for [`CatalogReviewId`].
pub enum CatalogReview {}
/// One All Data Reviewed approval of a Catalog Preview (REQ-OPS-015).
pub type CatalogReviewId = Id<CatalogReview>;

/// A draft version listed in a Catalog Preview, at the revision the report
/// printed (REQ-OPS-015).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "recordType", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum CatalogDraft {
    /// A method draft.
    MethodVersion {
        /// The version.
        id: MethodVersionId,
        /// Its revision when the report was generated.
        #[ts(type = "number")]
        revision: i64,
    },
    /// A test or panel draft.
    OrderableVersion {
        /// The version.
        id: OrderableVersionId,
        /// Its revision when the report was generated.
        #[ts(type = "number")]
        revision: i64,
    },
    /// A validity policy draft.
    ValidityPolicyVersion {
        /// The version.
        id: ValidityPolicyVersionId,
        /// Its revision when the report was generated.
        #[ts(type = "number")]
        revision: i64,
    },
}

impl CatalogDraft {
    /// The version's ID, whatever its kind.
    pub(crate) fn id_text(self) -> String {
        match self {
            Self::MethodVersion { id, .. } => id.to_string(),
            Self::OrderableVersion { id, .. } => id.to_string(),
            Self::ValidityPolicyVersion { id, .. } => id.to_string(),
        }
    }

    /// The same version, at any revision.
    pub(crate) fn same_record(self, other: Self) -> bool {
        self.id_text() == other.id_text()
    }
}

text_enum! {
    /// Lifecycle state of a method, orderable or validity policy version.
    #[ts(rename = "CatalogVersionStatus")]
    VersionStatus {
        /// Editable by its author; not usable.
        Draft = "draft",
        /// Immutable; superseded versions remain usable for accepted pinned work.
        Approved = "approved",
        /// Immutable and blocked for new use; history is unchanged.
        Retired = "retired",
    }
}

text_enum! {
    /// Whether a method's results are presumptive or definitive.
    MethodCertainty {
        /// Presumptive, such as an immunoassay screen.
        Presumptive = "presumptive",
        /// Definitive, such as mass spectrometry.
        Definitive = "definitive",
    }
}

text_enum! {
    /// Whether a method reports categories or quantities; independent of certainty.
    OutputScale {
        /// Categorical results.
        Qualitative = "qualitative",
        /// Numeric results with units.
        Quantitative = "quantitative",
    }
}

text_enum! {
    /// How a value compares with the cutoff to count as positive.
    ThresholdRule {
        /// Positive when the value is at or above the cutoff (≥).
        AtOrAbove = "at_or_above",
        /// Positive only when the value is above the cutoff (>).
        Above = "above",
    }
}

text_enum! {
    /// Whether the cutoff comparison uses the value before or after display rounding.
    ThresholdRounding {
        /// Compare the unrounded value.
        BeforeRounding = "before_rounding",
        /// Compare the value rounded to the display scale.
        AfterRounding = "after_rounding",
    }
}

text_enum! {
    /// Approved handling of numeric results against the analytical reportable range.
    #[derive(Default)]
    RangePolicy {
        /// Preserve supplied range information without an automatic review gate.
        #[default]
        ReportOnly = "report_only",
        /// Require an exact numeric final concentration within inclusive configured bounds.
        RequireWithinRange = "require_within_range",
    }
}

text_enum! {
    /// What an orderable is.
    OrderableKind {
        /// A single component.
        Test = "test",
        /// One or more components ordered together.
        Panel = "panel",
    }
}

text_enum! {
    /// What a component of an orderable measures. Orders copy it when they
    /// freeze the component, so validity evidence is told apart from drug
    /// results (REQ-SPC-004).
    ComponentPurpose {
        /// Drug or metabolite results.
        Analytical = "analytical",
        /// Specimen validity results, assessed against a validity policy.
        Validity = "validity",
    }
}

text_enum! {
    /// What an approved validity category requires (REQ-SPC-008).
    ValidityAction {
        /// No consequence beyond recording the category.
        None = "none",
        /// The report carries the category's qualifier text.
        Qualifier = "qualifier",
        /// Results are held for review.
        Hold = "hold",
        /// A new collection is requested.
        Recollect = "recollect",
        /// The report carries a comment.
        Comment = "comment",
    }
}

pub(crate) const CODE_MAX: usize = 64;
pub(crate) const NAME_MAX: usize = 200;
pub(crate) const REASON_MAX: usize = 1000;

/// A lab-defined code: letters, digits, `.`, `_` and `-`, starting with a
/// letter or digit. Codes are compared exactly.
pub(crate) fn code(field: &str, value: &str) -> Result<String, CatalogError> {
    let valid = value.len() <= CODE_MAX
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(value.to_owned())
    } else {
        Err(invalid(
            field,
            "invalid_code",
            "Use up to 64 letters, digits, '.', '_' or '-'.",
        ))
    }
}

/// Display text: trimmed, non-empty, at most `max` characters, no control characters.
pub(crate) fn text(field: &str, value: &str, max: usize) -> Result<String, CatalogError> {
    let trimmed = value.trim();
    match trimmed.chars().count() {
        0 => Err(invalid(field, "required", "Enter a value.")),
        n if n > max => Err(invalid(field, "too_long", "The value is too long.")),
        _ if trimmed.chars().any(char::is_control) => Err(invalid(
            field,
            "control_character",
            "Remove control characters.",
        )),
        _ => Ok(trimmed.to_owned()),
    }
}

/// Optional [`code`]: absent stays absent.
pub(crate) fn optional_code(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, CatalogError> {
    value.map(|value| code(field, value)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_restricted_and_exact() {
        for valid in ["UR", "AMPH-SCR", "loinc.1", "A_1", "9"] {
            assert_eq!(code("code", valid).unwrap(), valid);
        }
        for bad in ["", " UR", "UR ", "-UR", "a b", "Ü", &"X".repeat(65)] {
            assert!(code("code", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn text_is_trimmed_and_bounded() {
        assert_eq!(text("name", "  Urine ", 10).unwrap(), "Urine");
        assert!(text("name", "   ", 10).is_err());
        assert!(text("name", &"x".repeat(11), 10).is_err());
        assert!(text("name", "a\tb", 10).is_err());
    }

    #[test]
    fn codes_share_one_spelling_in_json_and_sql() {
        let json = serde_json::to_string(&ThresholdRule::AtOrAbove).unwrap();
        assert_eq!(json, "\"at_or_above\"");
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let back: ThresholdRounding = conn
            .query_row("SELECT ?1", [ThresholdRounding::AfterRounding], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(back, ThresholdRounding::AfterRounding);
        let unknown = conn.query_row("SELECT 'sometimes'", [], |row| {
            row.get::<_, ThresholdRule>(0)
        });
        assert!(unknown.is_err());
    }
}
