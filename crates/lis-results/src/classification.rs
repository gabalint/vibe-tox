//! Cutoff classification of a source value under the method version's
//! approved threshold configuration (REQ-RES-003). Comparison is exact
//! (`cmp_numeric`); nothing is rounded, converted or scaled here.

use std::cmp::Ordering;

use lis_catalog::{MethodAnalyte, ThresholdRounding, ThresholdRule};
use lis_core::{Comparator, DecimalText};

use crate::value::SourceValue;

/// How a value relates to the analyte's cutoff under the method's threshold rule.
///
/// `MeetsCutoff` means the value meets the rule: at or above the cutoff
/// for [`ThresholdRule::AtOrAbove`], strictly above it for
/// [`ThresholdRule::Above`]. `DoesNotMeetCutoff` means it does not, so a value equal
/// to the cutoff under `Above` is `DoesNotMeetCutoff`. This is a comparison with a
/// configured number, not a clinical interpretation (REQ-RES-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// The value meets the threshold rule.
    MeetsCutoff,
    /// The value does not meet the threshold rule.
    DoesNotMeetCutoff,
    /// A cutoff applies, but the answer is not certain from the value.
    NotClassifiable(NotClassifiableReason),
    /// No cutoff applies: the analyte has none, or the value is a qualitative code.
    NotApplicable,
}

impl Classification {
    /// The stable code stored with a result revision.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MeetsCutoff => "meets_cutoff",
            Self::DoesNotMeetCutoff => "does_not_meet_cutoff",
            Self::NotClassifiable(_) => "not_classifiable",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// Why a value with a cutoff could not be classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotClassifiableReason {
    /// The method compares after rounding, and rounding would be needed (or no
    /// display scale is set); no rounding mode is approved (R4, Q-03).
    RoundingRuleNotConfigured,
    /// The censored bound does not decide the comparison, including a bound
    /// equal to the cutoff under `<=` or `>=`.
    CensoredBoundInconclusive,
}

impl NotClassifiableReason {
    /// The stable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RoundingRuleNotConfigured => "rounding_rule_not_configured",
            Self::CensoredBoundInconclusive => "censored_bound_inconclusive",
        }
    }
}

/// Classifies `value` against the analyte's cutoff, using the method
/// version's threshold rule and rounding setting.
#[must_use]
pub fn classify(
    value: &SourceValue,
    rule: ThresholdRule,
    rounding: ThresholdRounding,
    analyte: &MethodAnalyte,
) -> Classification {
    let Some(cutoff) = &analyte.cutoff else {
        return Classification::NotApplicable;
    };
    let (comparator, magnitude) = match value {
        SourceValue::Qualitative(_) => return Classification::NotApplicable,
        SourceValue::Quantity(quantity) => (None, quantity),
        SourceValue::Censored { comparator, bound } => (Some(*comparator), bound),
    };
    if needs_rounding(magnitude, rounding, analyte.display_scale) {
        // REQ-RES-003, R4: never round without an approved rounding mode.
        return Classification::NotClassifiable(NotClassifiableReason::RoundingRuleNotConfigured);
    }
    let ordering = magnitude.cmp_numeric(cutoff);
    match comparator {
        None => exact(ordering, rule),
        Some(comparator) => censored(comparator, ordering),
    }
}

fn needs_rounding(
    magnitude: &DecimalText,
    rounding: ThresholdRounding,
    display: Option<u8>,
) -> bool {
    match rounding {
        ThresholdRounding::BeforeRounding => false,
        ThresholdRounding::AfterRounding => {
            display.is_none_or(|digits| magnitude.scale() > u32::from(digits))
        }
    }
}

/// `ordering` is the quantity compared with the cutoff.
fn exact(ordering: Ordering, rule: ThresholdRule) -> Classification {
    let meets = match rule {
        ThresholdRule::AtOrAbove => ordering.is_ge(),
        ThresholdRule::Above => ordering.is_gt(),
    };
    if meets {
        Classification::MeetsCutoff
    } else {
        Classification::DoesNotMeetCutoff
    }
}

/// `ordering` is the bound compared with the cutoff. A side is decided only
/// when every value the comparator allows lies on it, under either rule; an
/// equal bound under an inclusive comparator is never decided.
fn censored(comparator: Comparator, ordering: Ordering) -> Classification {
    let decided = match comparator {
        Comparator::Lt => ordering
            .is_le()
            .then_some(Classification::DoesNotMeetCutoff),
        Comparator::Le => ordering
            .is_lt()
            .then_some(Classification::DoesNotMeetCutoff),
        Comparator::Gt => ordering.is_ge().then_some(Classification::MeetsCutoff),
        Comparator::Ge => ordering.is_gt().then_some(Classification::MeetsCutoff),
    };
    decided.unwrap_or(Classification::NotClassifiable(
        NotClassifiableReason::CensoredBoundInconclusive,
    ))
}

#[cfg(test)]
mod tests;
