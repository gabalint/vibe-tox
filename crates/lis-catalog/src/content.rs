//! Versioned content inputs and their pure validation (REQ-ANA-002, NFR-10).
//!
//! Limits are lexical decimals ([`DecimalText`]): the text the laboratory
//! entered is stored and returned unchanged. Validation compares them
//! numerically, never as text, and never supplies a value.

use std::cmp::Ordering;

use lis_core::DecimalText;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CatalogError, invalid};
use crate::model::{
    AnalyteId, ComponentPurpose, MethodCertainty, MethodVersionId, NAME_MAX, OrderableKind,
    OutputScale, SpecimenTypeId, ThresholdRounding, ThresholdRule, ValidityAction, code, text,
};

/// Largest display scale (digits after the decimal point) a decimal can carry.
const DISPLAY_SCALE_MAX: u8 = 28;
const UNIT_MAX: usize = 64;

/// The content of a method version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MethodVersionContent {
    /// The specimen type the method is validated for.
    pub specimen_type_id: SpecimenTypeId,
    /// Presumptive or definitive.
    pub certainty: MethodCertainty,
    /// Qualitative or quantitative.
    pub output_scale: OutputScale,
    /// Positive at ≥ or at > the cutoff.
    pub threshold_rule: ThresholdRule,
    /// Whether the comparison uses the value before or after display rounding.
    pub threshold_rounding: ThresholdRounding,
    /// Laboratory-approved method information and limitations for patient reports.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub report_notes: Option<String>,
    /// The analytes the method measures, each with its own limits.
    pub analytes: Vec<MethodAnalyte>,
}

/// One analyte of a method version. Every limit is optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MethodAnalyte {
    /// The analyte.
    pub analyte_id: AnalyteId,
    /// Unit as UCUM text, as provided; never converted.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub unit: Option<String>,
    /// Cutoff.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub cutoff: Option<DecimalText>,
    /// Limit of detection.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub limit_of_detection: Option<DecimalText>,
    /// Limit of quantitation.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub limit_of_quantitation: Option<DecimalText>,
    /// Low end of the reportable range.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub reportable_low: Option<DecimalText>,
    /// High end of the reportable range.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub reportable_high: Option<DecimalText>,
    /// Report-only by default; opt in to an inclusive numeric range gate.
    #[serde(default)]
    pub range_policy: crate::model::RangePolicy,
    /// Laboratory-approved descriptive reference interval, distinct from analytical range.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub reference_interval: Option<String>,
    /// Digits shown after the decimal point.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub display_scale: Option<u8>,
}

impl MethodVersionContent {
    /// Checks the structure and the ordering of each analyte's limits, and
    /// trims units. Reference existence is checked against the database.
    pub(crate) fn validate(mut self) -> Result<Self, CatalogError> {
        self.report_notes =
            lis_core::optional_text("reportNotes", self.report_notes.as_deref(), 4000)
                .map_err(CatalogError::Invalid)?;
        if self.analytes.is_empty() {
            return Err(invalid("analytes", "required", "Add at least one analyte."));
        }
        for index in 0..self.analytes.len() {
            let field = format!("analytes[{index}]");
            let id = self.analytes[index].analyte_id;
            if self.analytes[..index].iter().any(|a| a.analyte_id == id) {
                return Err(invalid(
                    &field,
                    "duplicate",
                    "This analyte is listed twice.",
                ));
            }
            self.analytes[index].validate(&field)?;
            if self.analytes[index].range_policy == crate::model::RangePolicy::RequireWithinRange
                && (self.output_scale != OutputScale::Quantitative
                    || (self.analytes[index].reportable_low.is_none()
                        && self.analytes[index].reportable_high.is_none()))
            {
                return Err(invalid(
                    &format!("{field}.rangePolicy"),
                    "range_required",
                    "Range enforcement requires a quantitative method and at least one reportable bound.",
                ));
            }
        }
        Ok(self)
    }
}

impl MethodAnalyte {
    fn validate(&mut self, field: &str) -> Result<(), CatalogError> {
        self.reference_interval = lis_core::optional_text(
            &format!("{field}.referenceInterval"),
            self.reference_interval.as_deref(),
            1000,
        )
        .map_err(CatalogError::Invalid)?;
        self.unit = unit(&format!("{field}.unit"), self.unit.as_deref())?;
        if self
            .display_scale
            .is_some_and(|scale| scale > DISPLAY_SCALE_MAX)
        {
            return Err(invalid(
                &format!("{field}.displayScale"),
                "out_of_range",
                "Use a display scale from 0 to 28.",
            ));
        }
        ordered(
            &format!("{field}.reportableHigh"),
            self.reportable_low.as_ref(),
            self.reportable_high.as_ref(),
        )?;
        ordered(
            &format!("{field}.limitOfQuantitation"),
            self.limit_of_detection.as_ref(),
            self.limit_of_quantitation.as_ref(),
        )
    }
}

/// UCUM unit text, trimmed: blank is absent; at most 64 characters, no
/// control characters. Never converted.
pub(crate) fn unit(field: &str, value: Option<&str>) -> Result<Option<String>, CatalogError> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(unit) if unit.chars().count() > UNIT_MAX || unit.chars().any(char::is_control) => {
            Err(invalid(field, "invalid_unit", "Check the unit."))
        }
        Some(unit) => Ok(Some(unit.to_owned())),
    }
}

/// Refuses `high` numerically below `low` when both are present.
fn ordered(
    field: &str,
    low: Option<&DecimalText>,
    high: Option<&DecimalText>,
) -> Result<(), CatalogError> {
    match (low, high) {
        (Some(low), Some(high)) if high.cmp_numeric(low) == Ordering::Less => Err(invalid(
            field,
            "below_lower_limit",
            "This limit is below its lower counterpart.",
        )),
        _ => Ok(()),
    }
}

/// The component manifest of an orderable version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OrderableVersionContent {
    /// Components in order; positions are numbered from 1.
    pub components: Vec<ComponentInput>,
}

/// One component: a method version and the analytes it is expected to report.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(rename = "CatalogComponentInput")]
pub struct ComponentInput {
    /// A current approved method version, or an open method draft; approval
    /// of the orderable needs it current approved (REQ-ANA-009).
    pub method_version_id: MethodVersionId,
    /// Analytical or validity; chosen explicitly, never defaulted.
    pub purpose: ComponentPurpose,
    /// Analytes of that method version this component expects.
    pub analyte_ids: Vec<AnalyteId>,
}

impl OrderableVersionContent {
    /// Checks the structure: a test has exactly one component, a panel at
    /// least one; no method version or analyte repeats.
    pub(crate) fn validate(&self, kind: OrderableKind) -> Result<(), CatalogError> {
        component_count("components", kind, self.components.len())?;
        for (index, component) in self.components.iter().enumerate() {
            let field = format!("components[{index}]");
            if self.components[..index]
                .iter()
                .any(|c| c.method_version_id == component.method_version_id)
            {
                return Err(invalid(
                    &field,
                    "duplicate",
                    "This method version is listed twice.",
                ));
            }
            component.validate(&field)?;
        }
        Ok(())
    }
}

/// A test has exactly one component; a panel has at least one.
pub(crate) fn component_count(
    field: &str,
    kind: OrderableKind,
    count: usize,
) -> Result<(), CatalogError> {
    let count_ok = match kind {
        OrderableKind::Test => count == 1,
        OrderableKind::Panel => count >= 1,
    };
    if count_ok {
        Ok(())
    } else {
        Err(invalid(
            field,
            "component_count",
            "A test has one component; a panel has at least one.",
        ))
    }
}

impl ComponentInput {
    fn validate(&self, field: &str) -> Result<(), CatalogError> {
        if self.analyte_ids.is_empty() {
            return Err(invalid(
                &format!("{field}.analyteIds"),
                "required",
                "Add at least one analyte.",
            ));
        }
        for (index, id) in self.analyte_ids.iter().enumerate() {
            if self.analyte_ids[..index].contains(id) {
                return Err(invalid(
                    &format!("{field}.analyteIds[{index}]"),
                    "duplicate",
                    "This analyte is listed twice.",
                ));
            }
        }
        Ok(())
    }
}

/// The categories of a validity policy version (REQ-SPC-008).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValidityPolicyContent {
    /// The lab-approved categories, each mapped to one action.
    pub categories: Vec<ValidityCategory>,
}

/// One validity category as entered and as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValidityCategory {
    /// Lab-defined code, unique within the policy version.
    pub code: String,
    /// Display label.
    pub label: String,
    /// What the category requires.
    pub action: ValidityAction,
    /// The report qualifier; required for, and only for, the qualifier action.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub qualifier_text: Option<String>,
}

impl ValidityPolicyContent {
    /// Checks codes, labels and qualifier texts; trims labels and texts.
    pub(crate) fn validate(mut self) -> Result<Self, CatalogError> {
        if self.categories.is_empty() {
            return Err(invalid(
                "categories",
                "required",
                "Add at least one category.",
            ));
        }
        for index in 0..self.categories.len() {
            let field = format!("categories[{index}]");
            let category = &mut self.categories[index];
            category.code = code(&format!("{field}.code"), &category.code)?;
            category.label = text(&format!("{field}.label"), &category.label, NAME_MAX)?;
            category.qualifier_text = qualifier_text(&field, category)?;
            let code = &self.categories[index].code;
            if self.categories[..index].iter().any(|c| &c.code == code) {
                return Err(invalid(
                    &format!("{field}.code"),
                    "duplicate",
                    "This code is listed twice.",
                ));
            }
        }
        Ok(self)
    }
}

/// The trimmed qualifier text, required for and refused without the qualifier action.
fn qualifier_text(
    field: &str,
    category: &ValidityCategory,
) -> Result<Option<String>, CatalogError> {
    let field = format!("{field}.qualifierText");
    let given = category
        .qualifier_text
        .as_deref()
        .filter(|text| !text.trim().is_empty());
    match (category.action, given) {
        (ValidityAction::Qualifier, Some(value)) => text(&field, value, NAME_MAX).map(Some),
        (ValidityAction::Qualifier, None) => Err(invalid(
            &field,
            "required",
            "Enter the qualifier the report shows.",
        )),
        (_, Some(_)) => Err(invalid(
            &field,
            "not_allowed",
            "Only the qualifier action has qualifier text.",
        )),
        (_, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> Option<DecimalText> {
        Some(DecimalText::parse(text).unwrap())
    }

    fn analyte() -> MethodAnalyte {
        MethodAnalyte {
            reference_interval: None,
            range_policy: Default::default(),
            analyte_id: AnalyteId::generate(),
            unit: Some(" ng/mL ".to_owned()),
            cutoff: dec("111.10"),
            limit_of_detection: dec("2.0"),
            limit_of_quantitation: dec("2.00"),
            reportable_low: dec("5"),
            reportable_high: dec("999.900"),
            display_scale: Some(1),
        }
    }

    #[test]
    fn range_enforcement_requires_quantitative_method_and_configured_bounds() {
        let mut a = analyte();
        a.range_policy = crate::model::RangePolicy::RequireWithinRange;
        let mut c = content(vec![a.clone()]);
        assert!(c.clone().validate().is_ok());
        c.output_scale = OutputScale::Qualitative;
        assert!(c.validate().is_err());
        a.reportable_low = None;
        a.reportable_high = None;
        assert!(content(vec![a]).validate().is_err());
    }

    fn content(analytes: Vec<MethodAnalyte>) -> MethodVersionContent {
        MethodVersionContent {
            report_notes: None,
            specimen_type_id: SpecimenTypeId::generate(),
            certainty: MethodCertainty::Definitive,
            output_scale: OutputScale::Quantitative,
            threshold_rule: ThresholdRule::AtOrAbove,
            threshold_rounding: ThresholdRounding::BeforeRounding,
            analytes,
        }
    }

    fn field_of(error: CatalogError) -> String {
        let CatalogError::Invalid(field) = error else {
            panic!("{error:?}")
        };
        field.field
    }

    #[test]
    fn valid_content_keeps_decimal_text_and_trims_units() {
        let valid = content(vec![analyte()]).validate().unwrap();
        let a = &valid.analytes[0];
        assert_eq!(a.unit.as_deref(), Some("ng/mL"));
        assert_eq!(a.cutoff.as_ref().unwrap().as_str(), "111.10");
        assert_eq!(a.reportable_high.as_ref().unwrap().as_str(), "999.900");
    }

    #[test]
    fn limits_are_compared_numerically() {
        // "10" < "9" as text, but not as numbers.
        let mut a = analyte();
        a.reportable_low = dec("9");
        a.reportable_high = dec("10");
        assert!(content(vec![a]).validate().is_ok());

        let mut a = analyte();
        a.reportable_low = dec("10.0");
        a.reportable_high = dec("9.99");
        let error = content(vec![a]).validate().unwrap_err();
        assert_eq!(field_of(error), "analytes[0].reportableHigh");

        let mut a = analyte();
        a.limit_of_detection = dec("3");
        a.limit_of_quantitation = dec("2.999");
        let error = content(vec![a]).validate().unwrap_err();
        assert_eq!(field_of(error), "analytes[0].limitOfQuantitation");
    }

    #[test]
    fn structure_is_checked() {
        assert_eq!(
            field_of(content(vec![]).validate().unwrap_err()),
            "analytes"
        );
        let a = analyte();
        let error = content(vec![a.clone(), a]).validate().unwrap_err();
        assert_eq!(field_of(error), "analytes[1]");
        let mut a = analyte();
        a.display_scale = Some(29);
        let error = content(vec![a]).validate().unwrap_err();
        assert_eq!(field_of(error), "analytes[0].displayScale");
    }

    #[test]
    fn decimals_are_never_read_from_json_numbers() {
        let json = serde_json::json!({ "analyteId": AnalyteId::generate(), "cutoff": 50 });
        assert!(serde_json::from_value::<MethodAnalyte>(json).is_err());
    }

    #[test]
    fn component_counts_follow_the_orderable_kind() {
        let component = |mv: MethodVersionId| ComponentInput {
            method_version_id: mv,
            purpose: ComponentPurpose::Analytical,
            analyte_ids: vec![AnalyteId::generate()],
        };
        let one = OrderableVersionContent {
            components: vec![component(MethodVersionId::generate())],
        };
        let two = OrderableVersionContent {
            components: vec![
                component(MethodVersionId::generate()),
                component(MethodVersionId::generate()),
            ],
        };
        let none = OrderableVersionContent { components: vec![] };
        assert!(one.validate(OrderableKind::Test).is_ok());
        assert!(two.validate(OrderableKind::Test).is_err());
        assert!(two.validate(OrderableKind::Panel).is_ok());
        assert!(none.validate(OrderableKind::Panel).is_err());
        let mv = MethodVersionId::generate();
        let repeated = OrderableVersionContent {
            components: vec![component(mv), component(mv)],
        };
        assert_eq!(
            field_of(repeated.validate(OrderableKind::Panel).unwrap_err()),
            "components[1]"
        );
    }

    fn category(code: &str, action: ValidityAction, qualifier: Option<&str>) -> ValidityCategory {
        ValidityCategory {
            code: code.to_owned(),
            label: " Synthetic category ".to_owned(),
            action,
            qualifier_text: qualifier.map(str::to_owned),
        }
    }

    #[test]
    fn validity_categories_need_unique_codes_and_matching_qualifiers() {
        let valid = ValidityPolicyContent {
            categories: vec![
                category("SYN-OK", ValidityAction::None, None),
                category(
                    "SYN-DIL",
                    ValidityAction::Qualifier,
                    Some(" Synthetic qualifier "),
                ),
            ],
        }
        .validate()
        .unwrap();
        assert_eq!(valid.categories[0].label, "Synthetic category");
        assert_eq!(
            valid.categories[1].qualifier_text.as_deref(),
            Some("Synthetic qualifier")
        );

        let cases = [
            (vec![], "categories"),
            (
                vec![category("SYN-Q", ValidityAction::Qualifier, Some("  "))],
                "categories[0].qualifierText",
            ),
            (
                vec![category("SYN-H", ValidityAction::Hold, Some("text"))],
                "categories[0].qualifierText",
            ),
            (
                vec![
                    category("SYN-A", ValidityAction::Comment, None),
                    category("SYN-A", ValidityAction::Hold, None),
                ],
                "categories[1].code",
            ),
            (
                vec![category("bad code", ValidityAction::None, None)],
                "categories[0].code",
            ),
        ];
        for (categories, field) in cases {
            let error = ValidityPolicyContent { categories }.validate().unwrap_err();
            assert_eq!(field_of(error), field);
        }
    }
}
