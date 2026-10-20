use lis_catalog::AnalyteId;
use lis_catalog::OutputScale;
use lis_core::Decimal;
use proptest::prelude::*;

use super::*;
use crate::value::parse_value;

use Classification::{DoesNotMeetCutoff as NEG, MeetsCutoff as POS, NotApplicable as NA};
use ThresholdRounding::{AfterRounding as AFTER, BeforeRounding as BEFORE};
use ThresholdRule::{Above as GT, AtOrAbove as GE};

const NC_ROUND: Classification =
    Classification::NotClassifiable(NotClassifiableReason::RoundingRuleNotConfigured);
const NC_BOUND: Classification =
    Classification::NotClassifiable(NotClassifiableReason::CensoredBoundInconclusive);

fn analyte(cutoff: Option<&str>, display_scale: Option<u8>) -> MethodAnalyte {
    MethodAnalyte {
        reference_interval: None,
        range_policy: lis_catalog::RangePolicy::ReportOnly,
        analyte_id: AnalyteId::generate(),
        unit: Some("ng/mL".to_owned()),
        cutoff: cutoff.map(|text| DecimalText::parse(text).unwrap()),
        limit_of_detection: None,
        limit_of_quantitation: None,
        reportable_low: None,
        reportable_high: None,
        display_scale,
    }
}

fn quantity(text: &str) -> SourceValue {
    parse_value(text, OutputScale::Quantitative).unwrap()
}

/// The truth table, written from design brief §2.5 and the R4 default, not
/// from this implementation: (value, rule, rounding, cutoff, display scale,
/// expected classification).
#[rustfmt::skip]
struct Case(&'static str, ThresholdRule, ThresholdRounding, Option<&'static str>, Option<u8>, Classification);

#[rustfmt::skip]
const TRUTH_TABLE: &[Case] = &[
    // Exact quantities, compared before rounding.
    Case("49.99",  GE, BEFORE, Some("50"), None, NEG),
    Case("50",     GE, BEFORE, Some("50"), None, POS),
    Case("50.00",  GE, BEFORE, Some("50"), None, POS),
    Case("50.01",  GE, BEFORE, Some("50"), None, POS),
    Case("-1",     GE, BEFORE, Some("50"), None, NEG),
    Case("49.99",  GT, BEFORE, Some("50"), None, NEG),
    Case("50",     GT, BEFORE, Some("50"), None, NEG),
    Case("50.0",   GT, BEFORE, Some("50"), None, NEG),
    Case("50.01",  GT, BEFORE, Some("50"), None, POS),
    Case("0",      GE, BEFORE, Some("0"),  None, POS),
    Case("0.00",   GT, BEFORE, Some("0"),  None, NEG),
    Case("49.95",  GE, BEFORE, Some("50"), Some(1), NEG),
    // Censored: decided only when the bound makes the answer certain.
    Case("<50",    GE, BEFORE, Some("50"), None, NEG),
    Case("<50",    GT, BEFORE, Some("50"), None, NEG),
    Case("<49",    GE, BEFORE, Some("50"), None, NEG),
    Case("<51",    GE, BEFORE, Some("50"), None, NC_BOUND),
    Case("<51",    GT, BEFORE, Some("50"), None, NC_BOUND),
    Case("<=49.9", GE, BEFORE, Some("50"), None, NEG),
    Case("<=49.9", GT, BEFORE, Some("50"), None, NEG),
    Case("<=50",   GE, BEFORE, Some("50"), None, NC_BOUND),
    Case("<=50.0", GT, BEFORE, Some("50"), None, NC_BOUND),
    Case("<=51",   GE, BEFORE, Some("50"), None, NC_BOUND),
    Case(">50",    GE, BEFORE, Some("50"), None, POS),
    Case(">50.00", GT, BEFORE, Some("50"), None, POS),
    Case(">51",    GT, BEFORE, Some("50"), None, POS),
    Case(">49",    GE, BEFORE, Some("50"), None, NC_BOUND),
    Case(">49",    GT, BEFORE, Some("50"), None, NC_BOUND),
    Case(">=50",   GE, BEFORE, Some("50"), None, NC_BOUND),
    Case(">=50",   GT, BEFORE, Some("50"), None, NC_BOUND),
    Case(">=50.01",GE, BEFORE, Some("50"), None, POS),
    Case(">=50.01",GT, BEFORE, Some("50"), None, POS),
    Case(">=49",   GE, BEFORE, Some("50"), None, NC_BOUND),
    // Compared after rounding: classified only when no rounding is needed.
    Case("50.0",   GE, AFTER,  Some("50"), Some(1), POS),
    Case("49.9",   GE, AFTER,  Some("50"), Some(1), NEG),
    Case("50",     GT, AFTER,  Some("50"), Some(1), NEG),
    Case("49.95",  GE, AFTER,  Some("50"), Some(1), NC_ROUND),
    Case("50.04",  GT, AFTER,  Some("50"), Some(1), NC_ROUND),
    Case("<49.95", GE, AFTER,  Some("50"), Some(1), NC_ROUND),
    Case(">50.1",  GE, AFTER,  Some("50"), Some(1), POS),
    Case("50",     GE, AFTER,  Some("50"), Some(0), POS),
    Case("50.0",   GE, AFTER,  Some("50"), Some(0), NC_ROUND),
    Case("50",     GE, AFTER,  Some("50"), None, NC_ROUND),
    // No cutoff configured.
    Case("50",     GE, BEFORE, None, None, NA),
    Case("<0.5",   GT, BEFORE, None, None, NA),
    Case("49.95",  GE, AFTER,  None, None, NA),
];

#[test]
fn truth_table_matches_the_brief() {
    for &Case(text, rule, rounding, cutoff, display, expected) in TRUTH_TABLE {
        let actual = classify(&quantity(text), rule, rounding, &analyte(cutoff, display));
        assert_eq!(
            actual, expected,
            "{text} {rule:?} {rounding:?} cutoff {cutoff:?} display {display:?}"
        );
    }
}

#[test]
fn qualitative_code_is_not_applicable_even_with_a_cutoff() {
    let code = parse_value("POS", OutputScale::Qualitative).unwrap();
    for rule in [GE, GT] {
        assert_eq!(
            classify(&code, rule, BEFORE, &analyte(Some("50"), None)),
            NA
        );
    }
}

#[test]
fn codes_are_stable() {
    assert_eq!(POS.code(), "meets_cutoff");
    assert_eq!(NEG.code(), "does_not_meet_cutoff");
    assert_eq!(NC_ROUND.code(), "not_classifiable");
    assert_eq!(NA.code(), "not_applicable");
    assert_eq!(
        NotClassifiableReason::RoundingRuleNotConfigured.code(),
        "rounding_rule_not_configured"
    );
    assert_eq!(
        NotClassifiableReason::CensoredBoundInconclusive.code(),
        "censored_bound_inconclusive"
    );
}

fn decimal() -> impl Strategy<Value = Decimal> {
    (-1_000_000_000_i64..1_000_000_000, 0_u32..=4)
        .prop_map(|(units, scale)| Decimal::new(units, scale))
}

fn positive() -> impl Strategy<Value = Decimal> {
    (1_i64..1_000_000, 0_u32..=4).prop_map(|(units, scale)| Decimal::new(units, scale))
}

fn text(value: Decimal) -> DecimalText {
    DecimalText::parse(&value.to_string()).unwrap()
}

fn rule() -> impl Strategy<Value = ThresholdRule> {
    prop_oneof![Just(GE), Just(GT)]
}

fn comparator() -> impl Strategy<Value = Comparator> {
    prop_oneof![
        Just(Comparator::Lt),
        Just(Comparator::Le),
        Just(Comparator::Gt),
        Just(Comparator::Ge)
    ]
}

/// A value the censored form allows: below, at or above the bound as the
/// comparator permits.
fn allowed_value(
    comparator: Comparator,
    bound: Decimal,
    delta: Decimal,
    at_bound: bool,
) -> Decimal {
    match comparator {
        Comparator::Lt => bound - delta,
        Comparator::Le if at_bound => bound,
        Comparator::Le => bound - delta,
        Comparator::Gt => bound + delta,
        Comparator::Ge if at_bound => bound,
        Comparator::Ge => bound + delta,
    }
}

proptest! {
    #[test]
    fn a_quantity_above_the_cutoff_is_never_does_not_meet_cutoff(cutoff in decimal(), delta in positive(), rule in rule()) {
        let value = SourceValue::Quantity(text(cutoff + delta));
        let analyte = analyte(Some(text(cutoff).as_str()), None);
        prop_assert_eq!(classify(&value, rule, BEFORE, &analyte), POS);
    }

    #[test]
    fn a_quantity_below_the_cutoff_is_never_at_or_above(cutoff in decimal(), delta in positive(), rule in rule()) {
        let value = SourceValue::Quantity(text(cutoff - delta));
        let analyte = analyte(Some(text(cutoff).as_str()), None);
        prop_assert_eq!(classify(&value, rule, BEFORE, &analyte), NEG);
    }

    #[test]
    fn a_decided_censored_value_agrees_with_every_value_it_allows(
        comparator in comparator(),
        bound in decimal(),
        cutoff in decimal(),
        delta in positive(),
        at_bound in any::<bool>(),
        rule in rule(),
    ) {
        let analyte = analyte(Some(text(cutoff).as_str()), None);
        let censored = SourceValue::Censored { comparator, bound: text(bound) };
        let decided = classify(&censored, rule, BEFORE, &analyte);
        prop_assume!(decided != NC_BOUND);
        let value = SourceValue::Quantity(text(allowed_value(comparator, bound, delta, at_bound)));
        prop_assert_eq!(classify(&value, rule, BEFORE, &analyte), decided);
    }

    #[test]
    fn rounding_is_never_applied_after_rounding_methods(
        units in -1_000_000_i64..1_000_000,
        display in 0_u8..=3,
        extra in 1_u32..=3,
        cutoff in decimal(),
        rule in rule(),
    ) {
        let value = SourceValue::Quantity(text(Decimal::new(units, u32::from(display) + extra)));
        let analyte = analyte(Some(text(cutoff).as_str()), Some(display));
        prop_assert_eq!(classify(&value, rule, AFTER, &analyte), NC_ROUND);
    }
}
