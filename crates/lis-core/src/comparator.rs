//! Comparators of censored quantities such as `<0.50` (REQ-RES-001). The
//! comparator and its decimal bound are separate values; neither is folded
//! into the other, and the lexical form is what is stored and exchanged.

use std::fmt;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ParseError;

/// How a censored quantity relates to its bound. There is deliberately no
/// `=`: an exact quantity has no comparator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Comparator {
    /// Below the bound.
    #[serde(rename = "<")]
    Lt,
    /// At most the bound.
    #[serde(rename = "<=")]
    Le,
    /// Above the bound.
    #[serde(rename = ">")]
    Gt,
    /// At least the bound.
    #[serde(rename = ">=")]
    Ge,
}

impl Comparator {
    /// The lexical form: `<`, `<=`, `>` or `>=`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }

    /// Parses exactly one of the four lexical forms; nothing is trimmed.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        match text {
            "<" => Ok(Self::Lt),
            "<=" => Ok(Self::Le),
            ">" => Ok(Self::Gt),
            ">=" => Ok(Self::Ge),
            _ => Err(ParseError::Comparator),
        }
    }
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ToSql for Comparator {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

/// Reads TEXT only, in one of the four lexical forms.
impl FromSql for Comparator {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        // `as_str` refuses every storage class except TEXT.
        Self::parse(value.as_str()?).map_err(FromSqlError::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Comparator; 4] = [
        Comparator::Lt,
        Comparator::Le,
        Comparator::Gt,
        Comparator::Ge,
    ];

    #[test]
    fn lexical_form_round_trips_through_text_json_and_sqlite() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        for comparator in ALL {
            assert_eq!(Comparator::parse(comparator.as_str()), Ok(comparator));
            let json = serde_json::to_string(&comparator).unwrap();
            assert_eq!(json, format!("\"{}\"", comparator.as_str()));
            assert_eq!(
                serde_json::from_str::<Comparator>(&json).unwrap(),
                comparator
            );
            let stored: Comparator = conn
                .query_row("SELECT ?1", [comparator], |row| row.get(0))
                .unwrap();
            assert_eq!(stored, comparator);
        }
    }

    #[test]
    fn anything_else_is_refused_without_trimming() {
        for text in ["", "=", "==", "=<", " <", "< ", "≤", "lt", "<<"] {
            assert_eq!(
                Comparator::parse(text),
                Err(ParseError::Comparator),
                "{text:?}"
            );
        }
    }

    #[test]
    fn sqlite_rejects_non_text_values() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let stored: rusqlite::Result<Comparator> = conn.query_row("SELECT 1", [], |row| row.get(0));
        assert!(stored.is_err());
    }
}
