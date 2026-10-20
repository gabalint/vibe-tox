//! The value a result revision records from its evidence (REQ-RES-001,
//! REQ-RES-002), and the one home of what an approved value word asserts
//! (result files brief F7, Q-40): the word stays the measurement; only its
//! approved rule turns it into a reportable result, and the revision cites
//! that rule. A word never applies on a dilution execution (F7, "Dilutions").

use lis_analytical::{SourceRegistrationId, ValueWordMeaning};
use lis_catalog::{MethodAnalyte, MethodVersionView};
use lis_core::{Comparator, Decimal, DecimalText};

use crate::classification::{Classification, classify};
use crate::error::{ResultError, gate};
use crate::model::{QuarantineReason, ResultOutcome};
use crate::value::SourceValue;

/// The approved rule a word-derived result cites: the registration version
/// whose template maps the word, and the word as received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValueRule {
    pub(crate) registration_id: SourceRegistrationId,
    pub(crate) word: String,
}

/// Why an approved word asserts no result for a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordRefusal {
    /// The execution is a dilution. The method version's limit is the
    /// undiluted one, not the limit of the diluted injection, and the LIS
    /// never computes a diluted bound.
    Diluted,
    /// The frozen method version lacks the limit the word's meaning needs.
    MissingLimit,
}

impl WordRefusal {
    /// The reason an import quarantines (or sets aside) the row with.
    pub(crate) const fn quarantine_reason(self) -> QuarantineReason {
        match self {
            Self::Diluted => QuarantineReason::DilutionNotSupported,
            Self::MissingLimit => QuarantineReason::MalformedValue,
        }
    }

    /// The refusal of a reviewer's command that would make a result of it.
    pub(crate) fn error(self) -> ResultError {
        match self {
            Self::Diluted => ResultError::GateBlocked(vec![gate(
                QuarantineReason::DilutionNotSupported.as_str(),
                "A value word does not apply to a dilution: the method's limit is not the \
                 diluted injection's.",
            )]),
            Self::MissingLimit => ResultError::WrongState("method limit"),
        }
    }
}

/// An execution is a dilution when it records a factor that is not
/// numerically 1.
pub(crate) fn diluted(factor: Option<&DecimalText>) -> bool {
    factor.is_some_and(|factor| factor.to_decimal() != Decimal::ONE)
}

/// What an approved word asserts for `analyte`, measured by an execution with
/// the dilution `factor`: below the LOQ or the LOD is a censored `<` value
/// bounded by that limit of the frozen method version; not performed asserts
/// no value (`None`). The bound comes from the method version, never from the
/// file. No word applies on a dilution.
pub(crate) fn word_result(
    meaning: ValueWordMeaning,
    analyte: &MethodAnalyte,
    factor: Option<&DecimalText>,
) -> Result<Option<SourceValue>, WordRefusal> {
    if diluted(factor) {
        return Err(WordRefusal::Diluted);
    }
    let limit = match meaning {
        ValueWordMeaning::BelowLoq => analyte.limit_of_quantitation.as_ref(),
        ValueWordMeaning::BelowLod => analyte.limit_of_detection.as_ref(),
        ValueWordMeaning::NotPerformed => return Ok(None),
    };
    let bound = limit.cloned().ok_or(WordRefusal::MissingLimit)?;
    Ok(Some(SourceValue::Censored {
        comparator: Comparator::Lt,
        bound,
    }))
}

/// The value one result revision records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResultValue {
    /// The asserted value; `None` when the test was not performed.
    pub(crate) value: Option<SourceValue>,
    /// The unit text recorded with the value.
    pub(crate) units: Option<String>,
    /// The approved word rule, for a value derived from a word.
    pub(crate) rule: Option<ValueRule>,
}

/// The stored columns of a revision value: outcome, comparator, magnitude and code.
pub(crate) type ValueColumns<'a> = (
    ResultOutcome,
    Option<Comparator>,
    Option<&'a str>,
    Option<&'a str>,
);

impl ResultValue {
    /// A word-derived value: the analyte's unit, and the rule it cites; see
    /// [`word_result`].
    pub(crate) fn from_word(
        meaning: ValueWordMeaning,
        analyte: &MethodAnalyte,
        rule: ValueRule,
        factor: Option<&DecimalText>,
    ) -> Result<Self, WordRefusal> {
        Ok(Self {
            value: word_result(meaning, analyte, factor)?,
            units: analyte.unit.clone(),
            rule: Some(rule),
        })
    }

    pub(crate) fn columns(&self) -> ValueColumns<'_> {
        match &self.value {
            None => (ResultOutcome::NotPerformed, None, None, None),
            Some(SourceValue::Quantity(value)) => {
                (ResultOutcome::Quantity, None, Some(value.as_str()), None)
            }
            Some(SourceValue::Censored { comparator, bound }) => (
                ResultOutcome::Censored,
                Some(*comparator),
                Some(bound.as_str()),
                None,
            ),
            Some(SourceValue::Qualitative(code)) => {
                (ResultOutcome::Qualitative, None, None, Some(code.as_str()))
            }
        }
    }

    /// The cutoff comparison of the value under the frozen method version
    /// (REQ-RES-003); a test not performed has no classification.
    pub(crate) fn classification(
        &self,
        method: &MethodVersionView,
        analyte: &MethodAnalyte,
    ) -> Classification {
        self.value
            .as_ref()
            .map_or(Classification::NotApplicable, |value| {
                classify(
                    value,
                    method.threshold_rule,
                    method.threshold_rounding,
                    analyte,
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use lis_catalog::AnalyteId;
    use lis_core::DecimalText;

    use super::*;

    fn analyte(loq: Option<&str>, lod: Option<&str>) -> MethodAnalyte {
        MethodAnalyte {
            reference_interval: None,
            range_policy: lis_catalog::RangePolicy::ReportOnly,
            analyte_id: AnalyteId::generate(),
            unit: Some("ng/mL".to_owned()),
            cutoff: Some(DecimalText::parse("50").unwrap()),
            limit_of_detection: lod.map(|text| DecimalText::parse(text).unwrap()),
            limit_of_quantitation: loq.map(|text| DecimalText::parse(text).unwrap()),
            reportable_low: None,
            reportable_high: None,
            display_scale: Some(1),
        }
    }

    fn below(bound: &str) -> Option<SourceValue> {
        Some(SourceValue::Censored {
            comparator: Comparator::Lt,
            bound: DecimalText::parse(bound).unwrap(),
        })
    }

    fn factor(text: &str) -> DecimalText {
        DecimalText::parse(text).unwrap()
    }

    // F7: the bound is the method version's limit, never a number from the file.
    #[test]
    fn a_word_below_a_limit_is_censored_at_that_limit_of_the_method_version() {
        let limits = analyte(Some("5.0"), Some("1.0"));
        assert_eq!(
            word_result(ValueWordMeaning::BelowLoq, &limits, None),
            Ok(below("5.0"))
        );
        assert_eq!(
            word_result(ValueWordMeaning::BelowLod, &limits, None),
            Ok(below("1.0"))
        );
        assert_eq!(
            word_result(ValueWordMeaning::NotPerformed, &limits, None),
            Ok(None)
        );
    }

    #[test]
    fn a_word_whose_limit_the_method_version_lacks_has_no_result() {
        let none = analyte(None, None);
        assert_eq!(
            word_result(ValueWordMeaning::BelowLoq, &none, None),
            Err(WordRefusal::MissingLimit)
        );
        assert_eq!(
            word_result(ValueWordMeaning::BelowLod, &none, None),
            Err(WordRefusal::MissingLimit)
        );
        assert_eq!(
            word_result(ValueWordMeaning::NotPerformed, &none, None),
            Ok(None)
        );
    }

    // F7 "Dilutions" (H1 of the wave-10 review): no word applies on a
    // dilution, whatever it means; a factor numerically 1 is no dilution.
    #[test]
    fn no_word_applies_on_a_dilution_execution() {
        let limits = analyte(Some("5.0"), Some("1.0"));
        for meaning in [
            ValueWordMeaning::BelowLoq,
            ValueWordMeaning::BelowLod,
            ValueWordMeaning::NotPerformed,
        ] {
            for diluted in ["10", "0.5", "2.0"] {
                assert_eq!(
                    word_result(meaning, &limits, Some(&factor(diluted))),
                    Err(WordRefusal::Diluted),
                    "{diluted}"
                );
            }
            for undiluted in ["1", "1.000"] {
                assert!(word_result(meaning, &limits, Some(&factor(undiluted))).is_ok());
            }
        }
        assert_eq!(
            WordRefusal::Diluted.quarantine_reason(),
            QuarantineReason::DilutionNotSupported
        );
        assert_eq!(
            WordRefusal::Diluted.error().gate_reasons()[0].code,
            "dilution_not_supported"
        );
    }
}
