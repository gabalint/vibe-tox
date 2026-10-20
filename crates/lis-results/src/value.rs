//! Source result values as typed forms (REQ-RES-001, NFR-10). The text is
//! read exactly as received: nothing is trimmed, rounded or coerced.

use lis_catalog::OutputScale;
use lis_core::{Comparator, DecimalText, ParseError};

/// A result value the source asserted, in one of its value forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceValue {
    /// An exact quantity, such as `12.50`.
    Quantity(DecimalText),
    /// A censored quantity, such as `<0.50`: the value relates to the bound
    /// by the comparator; the bound is not the value.
    Censored {
        /// How the value relates to the bound.
        comparator: Comparator,
        /// The bound, exactly as written.
        bound: DecimalText,
    },
    /// A categorical code from a qualitative method, exactly as received.
    Qualitative(String),
}

/// Why a value text could not be read; the codes are stored with quarantine items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalformedValue {
    /// The text is empty.
    Empty,
    /// The text is not a plain decimal or a comparator immediately followed by one.
    NotADecimal,
    /// The decimal is outside the exact range or precision supported.
    DecimalOutOfRange,
}

impl MalformedValue {
    /// The stable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::NotADecimal => "not_a_decimal",
            Self::DecimalOutOfRange => "decimal_out_of_range",
        }
    }
}

/// Reads a source value text under the method's output scale.
///
/// - Quantitative: `-?d+(.d+)?` is a quantity; `<`, `<=`, `>` or `>=`
///   immediately followed by such a decimal is a censored value. Anything
///   else, including surrounding spaces, `+`, exponents or a comma, is malformed.
/// - Qualitative: any non-empty text is the code, exactly as received.
pub fn parse_value(text: &str, scale: OutputScale) -> Result<SourceValue, MalformedValue> {
    if text.is_empty() {
        return Err(MalformedValue::Empty);
    }
    match scale {
        OutputScale::Qualitative => Ok(SourceValue::Qualitative(text.to_owned())),
        OutputScale::Quantitative => parse_quantity(text),
    }
}

fn parse_quantity(text: &str) -> Result<SourceValue, MalformedValue> {
    // Two-character comparators first, so `<=` is never read as `<` and `=…`.
    let censored = [
        Comparator::Le,
        Comparator::Ge,
        Comparator::Lt,
        Comparator::Gt,
    ]
    .into_iter()
    .find_map(|comparator| Some((comparator, text.strip_prefix(comparator.as_str())?)));
    let Some((comparator, bound)) = censored else {
        return decimal(text).map(SourceValue::Quantity);
    };
    Ok(SourceValue::Censored {
        comparator,
        bound: decimal(bound)?,
    })
}

fn decimal(text: &str) -> Result<DecimalText, MalformedValue> {
    DecimalText::parse(text).map_err(|error| match error {
        ParseError::DecimalRange => MalformedValue::DecimalOutOfRange,
        ParseError::DecimalSyntax
        | ParseError::Id
        | ParseError::Timestamp
        | ParseError::Comparator => MalformedValue::NotADecimal,
    })
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn quantitative(text: &str) -> Result<SourceValue, MalformedValue> {
        parse_value(text, OutputScale::Quantitative)
    }

    fn censored(comparator: Comparator, bound: &str) -> SourceValue {
        SourceValue::Censored {
            comparator,
            bound: DecimalText::parse(bound).unwrap(),
        }
    }

    #[test]
    fn quantity_text_is_kept_exactly_including_trailing_zeros() {
        for text in ["0", "12", "12.50", "-0.003", "007.10"] {
            let Ok(SourceValue::Quantity(value)) = quantitative(text) else {
                panic!("{text} is a quantity");
            };
            assert_eq!(value.as_str(), text);
        }
    }

    #[test]
    fn each_comparator_followed_by_a_decimal_is_censored() {
        assert_eq!(quantitative("<0.50"), Ok(censored(Comparator::Lt, "0.50")));
        assert_eq!(quantitative("<=25"), Ok(censored(Comparator::Le, "25")));
        assert_eq!(quantitative(">1000"), Ok(censored(Comparator::Gt, "1000")));
        assert_eq!(quantitative(">=3.0"), Ok(censored(Comparator::Ge, "3.0")));
    }

    #[test]
    fn nothing_is_trimmed_or_coerced() {
        let cases = [
            ("", MalformedValue::Empty),
            (" 12", MalformedValue::NotADecimal),
            ("12 ", MalformedValue::NotADecimal),
            ("< 5", MalformedValue::NotADecimal),
            ("<", MalformedValue::NotADecimal),
            ("=5", MalformedValue::NotADecimal),
            ("=<5", MalformedValue::NotADecimal),
            ("<<5", MalformedValue::NotADecimal),
            ("+5", MalformedValue::NotADecimal),
            ("1e3", MalformedValue::NotADecimal),
            ("1,5", MalformedValue::NotADecimal),
            (".5", MalformedValue::NotADecimal),
            ("5.", MalformedValue::NotADecimal),
            ("NEG", MalformedValue::NotADecimal),
            ("NaN", MalformedValue::NotADecimal),
            ("5<", MalformedValue::NotADecimal),
            (
                "79228162514264337593543950336",
                MalformedValue::DecimalOutOfRange,
            ),
            (
                "<79228162514264337593543950336",
                MalformedValue::DecimalOutOfRange,
            ),
        ];
        for (text, reason) in cases {
            assert_eq!(quantitative(text), Err(reason), "{text:?}");
        }
    }

    #[test]
    fn qualitative_scale_keeps_any_text_as_the_code() {
        for text in ["NEG", "POS", " POS", "12.5", "<5", "Not detected"] {
            assert_eq!(
                parse_value(text, OutputScale::Qualitative),
                Ok(SourceValue::Qualitative(text.to_owned()))
            );
        }
        assert_eq!(
            parse_value("", OutputScale::Qualitative),
            Err(MalformedValue::Empty)
        );
    }

    #[test]
    fn reason_codes_are_stable() {
        assert_eq!(MalformedValue::Empty.code(), "empty");
        assert_eq!(MalformedValue::NotADecimal.code(), "not_a_decimal");
        assert_eq!(
            MalformedValue::DecimalOutOfRange.code(),
            "decimal_out_of_range"
        );
    }

    proptest! {
        #[test]
        fn arbitrary_text_never_panics_and_round_trips_when_read(text in any::<String>()) {
            for scale in [OutputScale::Quantitative, OutputScale::Qualitative] {
                let written = match parse_value(&text, scale) {
                    Ok(SourceValue::Quantity(value)) => value.as_str().to_owned(),
                    Ok(SourceValue::Censored { comparator, bound }) => {
                        format!("{comparator}{}", bound.as_str())
                    }
                    Ok(SourceValue::Qualitative(code)) => code,
                    Err(_) => continue,
                };
                // REQ-RES-001: the typed form reproduces the received text exactly.
                prop_assert_eq!(&written, &text);
            }
        }

        #[test]
        fn any_written_decimal_reads_back_as_that_quantity(
            negative in any::<bool>(),
            integer in "[0-9]{1,12}",
            fraction in proptest::option::of("[0-9]{1,8}"),
        ) {
            let sign = if negative { "-" } else { "" };
            let text = match fraction {
                Some(fraction) => format!("{sign}{integer}.{fraction}"),
                None => format!("{sign}{integer}"),
            };
            prop_assert_eq!(
                quantitative(&text),
                Ok(SourceValue::Quantity(DecimalText::parse(&text).unwrap()))
            );
        }
    }
}
