//! Lexical decimal quantities (NFR-10, ADR 0012).
//!
//! A [`DecimalText`] keeps the exact text of a quantity as received or entered,
//! such as `"1.50"`, `"-0.003"` or `"120"`. The text is the source value; the
//! exact [`Decimal`] is derived from it for comparison and arithmetic.
//!
//! **Lexical and numeric equality differ.** `"1.5"` and `"1.50"` are different
//! values (`==` is false), because the trailing zero records the reported
//! precision. They are numerically equal (`cmp_numeric` returns `Equal`). Never
//! use `==` for a clinical threshold, and never compare the SQLite TEXT column
//! in SQL for one.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ParseError;

/// An exact decimal held as its original text.
///
/// Accepted text: an optional leading `-`, one or more ASCII digits, and an
/// optional `.` followed by one or more digits. Exponents, `NaN`, infinities,
/// `+`, separators, whitespace and the empty string are rejected, as is any
/// value outside the range or scale of `rust_decimal`. Nothing is ever rounded.
#[derive(Clone)]
pub struct DecimalText {
    text: Box<str>,
    value: Decimal,
}

impl DecimalText {
    /// Validates `text` and keeps it exactly as given.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let fraction_digits = check_syntax(text)?;
        let value = Decimal::from_str_exact(text).map_err(|_| ParseError::DecimalRange)?;
        // Belt and braces: the parsed scale must equal the written scale.
        if usize::try_from(value.scale()).ok() != Some(fraction_digits) {
            return Err(ParseError::DecimalRange);
        }
        Ok(Self {
            text: text.into(),
            value,
        })
    }

    /// The exact text as received.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The number of digits after the decimal point in the text (`"1.50"` → 2).
    #[must_use]
    pub fn scale(&self) -> u32 {
        self.value.scale()
    }

    /// The exact numeric value, with the text's scale.
    #[must_use]
    pub fn to_decimal(&self) -> Decimal {
        self.value
    }

    /// Compares numeric values: `"1.5"` and `"1.50"` are `Equal`.
    #[must_use]
    pub fn cmp_numeric(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

/// Returns the number of fraction digits when `text` has the accepted syntax.
fn check_syntax(text: &str) -> Result<usize, ParseError> {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (unsigned, None),
    };
    let all_digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(integer) || fraction.is_some_and(|part| !all_digits(part)) {
        return Err(ParseError::DecimalSyntax);
    }
    Ok(fraction.map_or(0, str::len))
}

/// Lexical equality: the texts are identical.
impl PartialEq for DecimalText {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for DecimalText {}

impl Hash for DecimalText {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl fmt::Debug for DecimalText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecimalText({:?})", self.text)
    }
}

impl fmt::Display for DecimalText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl FromStr for DecimalText {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl Serialize for DecimalText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text)
    }
}

/// Accepts a JSON string only; a JSON number is an error (NFR-10).
impl<'de> Deserialize<'de> for DecimalText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl ts_rs::TS for DecimalText {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "string".to_owned()
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        Self::name(cfg)
    }
}

impl ToSql for DecimalText {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

/// Reads TEXT only; an INTEGER or REAL value means the column lost the lexical form.
impl FromSql for DecimalText {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        // `as_str` refuses every storage class except TEXT.
        Self::parse(value.as_str()?).map_err(FromSqlError::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn dec(text: &str) -> DecimalText {
        DecimalText::parse(text).unwrap()
    }

    #[test]
    fn keeps_text_and_scale_exactly() {
        for (text, scale) in [("1.50", 2), ("-0.003", 3), ("120", 0), ("0", 0)] {
            let value = dec(text);
            assert_eq!(value.as_str(), text);
            assert_eq!(value.scale(), scale);
            assert_eq!(value.to_decimal().to_string(), text);
        }
    }

    #[test]
    fn negative_zero_keeps_its_text() {
        // rust_decimal drops the sign of zero; the lexical text keeps it.
        let value = dec("-0.0");
        assert_eq!(value.as_str(), "-0.0");
        assert_eq!(value.scale(), 1);
        assert!(value.to_decimal().is_zero());
    }

    #[test]
    fn rejects_malformed_text() {
        for text in [
            "", "-", ".", ".5", "5.", "+1", "1e3", "1E-2", "NaN", "inf", "-inf", "1,5", "1 000",
            " 1", "1 ", "1.2.3", "--1", "0x10", "1_000", "١٢", "1.5\n",
        ] {
            assert_eq!(
                DecimalText::parse(text),
                Err(ParseError::DecimalSyntax),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_values_outside_rust_decimal_without_rounding() {
        // Scale above 28.
        let tiny = format!("0.{}1", "0".repeat(28));
        // 29 significant digits at scale 28 exceed the 96-bit mantissa.
        let wide = format!("9.{}", "9".repeat(28));
        // Integer part above 2^96 - 1.
        let huge = "79228162514264337593543950336";
        for text in [tiny.as_str(), wide.as_str(), huge] {
            assert_eq!(
                DecimalText::parse(text),
                Err(ParseError::DecimalRange),
                "{text}"
            );
        }
        assert!(DecimalText::parse("79228162514264337593543950335").is_ok());
        assert!(DecimalText::parse(&format!("0.{}1", "0".repeat(27))).is_ok());
    }

    #[test]
    fn lexical_and_numeric_equality_differ() {
        assert_ne!(dec("1.5"), dec("1.50"));
        assert_eq!(dec("1.5").cmp_numeric(&dec("1.50")), Ordering::Equal);
        assert_eq!(dec("-0").cmp_numeric(&dec("0.00")), Ordering::Equal);
        assert_eq!(dec("2").cmp_numeric(&dec("10")), Ordering::Less);
        assert_eq!(dec("-1.01").cmp_numeric(&dec("-1.1")), Ordering::Greater);
    }

    #[test]
    fn json_accepts_strings_only() {
        let value: DecimalText = serde_json::from_str("\"1.50\"").unwrap();
        assert_eq!(value.as_str(), "1.50");
        assert_eq!(serde_json::to_string(&value).unwrap(), "\"1.50\"");
        for json in ["1.5", "1.50", "120", "-0.003", "1e3", "null", "\"1e3\""] {
            assert!(serde_json::from_str::<DecimalText>(json).is_err(), "{json}");
        }
    }

    #[test]
    fn trailing_zeros_survive_a_strict_text_column() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE q (v TEXT NOT NULL) STRICT")
            .unwrap();
        for text in ["1.50", "120", "0.000", "-0.0", "100.10"] {
            let back: DecimalText = conn
                .query_row("SELECT ?1", [dec(text)], |row| row.get(0))
                .unwrap();
            conn.execute("INSERT INTO q (v) VALUES (?1)", [dec(text)])
                .unwrap();
            assert_eq!(back.as_str(), text);
        }
        let stored: Vec<String> = conn
            .prepare("SELECT v FROM q")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(stored, ["1.50", "120", "0.000", "-0.0", "100.10"]);
    }

    #[test]
    fn sqlite_rejects_non_text_values() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        for sql in ["SELECT 1.5", "SELECT 15"] {
            let read = conn.query_row(sql, [], |row| row.get::<_, DecimalText>(0));
            assert!(read.is_err(), "{sql}");
        }
    }

    #[test]
    fn typescript_type_is_string() {
        use ts_rs::TS;
        assert_eq!(DecimalText::inline(&ts_rs::Config::default()), "string");
    }

    fn decimal_text() -> impl Strategy<Value = String> {
        // Up to 28 significant digits always fit, at any scale up to 28.
        ("-?", "[0-9]{1,14}", proptest::option::of("[0-9]{1,14}")).prop_map(
            |(sign, integer, fraction)| match fraction {
                Some(fraction) => format!("{sign}{integer}.{fraction}"),
                None => format!("{sign}{integer}"),
            },
        )
    }

    proptest! {
        // ADR 0012 acceptance item 1, NFR-10: text → DecimalText → SQLite TEXT → back,
        // and through JSON, keeps the exact text including trailing zeros.
        #[test]
        fn round_trip_through_sqlite_and_json_preserves_text(text in decimal_text()) {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch("CREATE TABLE q (v TEXT NOT NULL) STRICT").unwrap();
            let value = dec(&text);
            conn.execute("INSERT INTO q (v) VALUES (?1)", [&value]).unwrap();
            let (stored, back): (String, DecimalText) = conn
                .query_row("SELECT v, v FROM q", [], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap();
            prop_assert_eq!(&stored, &text);
            prop_assert_eq!(back.as_str(), text.as_str());
            prop_assert_eq!(back.scale(), value.scale());
            let json = serde_json::to_string(&value).unwrap();
            let from_json: DecimalText = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(from_json.as_str(), text.as_str());
        }

        #[test]
        fn parse_never_panics(text in "\\PC{0,40}") {
            let _ = DecimalText::parse(&text);
        }
    }
}
