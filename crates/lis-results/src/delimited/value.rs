//! The value rules of result files (brief F7, Q-40): the one home of the
//! word table, the empty rule and the zero rule.

use lis_analytical::{ValueWord, ValueWordMeaning};
use lis_catalog::OutputScale;
use lis_core::Decimal;

use crate::value::{MalformedValue, SourceValue, parse_value};

/// How one value text of a result file reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileValueReading {
    /// A positive quantity, a censored value with a positive bound, or a
    /// qualitative code, exactly as the existing reader reads it.
    Value(SourceValue),
    /// The exact text is an approved word with this meaning.
    Word(ValueWordMeaning),
    /// The field is empty: never mapped, never a negative result.
    Empty,
    /// Non-empty text without a digit that no approved word matches, such
    /// as an unmapped `N/F`.
    UnmappedWord,
    /// A quantity of zero or below, or a comparator with a bound of zero or
    /// below (`<0`, `>0`), that no approved word matches.
    ZeroOrNegative,
    /// Text that is neither a value nor a word, such as `1,234.5`.
    Malformed(MalformedValue),
}

/// Reads a file's value text for a method of `scale` (F7), in this order:
///
/// 1. an approved word whose text equals `text` exactly gives its meaning;
///    a qualitative method takes only "not performed", and reads any other
///    word as its code;
/// 2. empty text is [`FileValueReading::Empty`];
/// 3. the strict reader ([`parse_value`]) reads the rest; nothing is trimmed;
/// 4. a quantity or comparator bound of zero or below is
///    [`FileValueReading::ZeroOrNegative`], compared numerically.
#[must_use]
pub fn read_value(text: &str, words: &[ValueWord], scale: OutputScale) -> FileValueReading {
    let meaning = words
        .iter()
        .find(|word| word.word == text)
        .map(|word| word.meaning)
        .filter(|meaning| {
            scale == OutputScale::Quantitative || *meaning == ValueWordMeaning::NotPerformed
        });
    if let Some(meaning) = meaning {
        return FileValueReading::Word(meaning);
    }
    if text.is_empty() {
        return FileValueReading::Empty;
    }
    match parse_value(text, scale) {
        Ok(SourceValue::Quantity(value)) if value.to_decimal() <= Decimal::ZERO => {
            FileValueReading::ZeroOrNegative
        }
        Ok(SourceValue::Censored { bound, .. }) if bound.to_decimal() <= Decimal::ZERO => {
            FileValueReading::ZeroOrNegative
        }
        Ok(value) => FileValueReading::Value(value),
        Err(MalformedValue::NotADecimal) if !text.bytes().any(|byte| byte.is_ascii_digit()) => {
            FileValueReading::UnmappedWord
        }
        Err(reason) => FileValueReading::Malformed(reason),
    }
}
