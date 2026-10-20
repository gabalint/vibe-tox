//! UTC instants and the injectable clock.
//!
//! [`UtcTimestamp`] has one canonical text form: RFC 3339 in UTC with exactly
//! nine fractional digits, for example `2026-10-12T14:03:07.123456789Z`. The
//! fixed width makes lexical order equal chronological order, so SQLite TEXT
//! columns sort and compare correctly (for years 0000–9999, the only years the
//! host clock produces). Nothing is truncated: the form carries the full
//! nanosecond precision of `jiff::Timestamp`.

use std::fmt;
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ParseError;

/// An instant in UTC with nanosecond precision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcTimestamp(jiff::Timestamp);

impl UtcTimestamp {
    /// Parses the canonical form only (see the module documentation).
    ///
    /// Offsets other than `Z`, other fraction widths and other RFC 3339
    /// variants are rejected, so a source offset is never dropped silently.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let parsed = Self(text.parse().map_err(|_| ParseError::Timestamp)?);
        // A leading digit excludes negative years, which would not sort as text.
        if parsed.to_string() != text || !text.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(ParseError::Timestamp);
        }
        Ok(parsed)
    }

    /// `self + duration`, or `None` beyond the supported range.
    #[must_use]
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration).ok().map(Self)
    }

    /// The time elapsed since `earlier`, or zero if `earlier` is later.
    #[must_use]
    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::try_from(self.0.duration_since(earlier.0)).unwrap_or(Duration::ZERO)
    }

    /// Whole seconds since 1970-01-01T00:00:00Z, rounded towards zero (RFC 6238 time).
    #[must_use]
    pub fn unix_seconds(self) -> i64 {
        self.0.as_second()
    }
}

impl fmt::Debug for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UtcTimestamp({self})")
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.9}", self.0)
    }
}

impl FromStr for UtcTimestamp {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl Serialize for UtcTimestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for UtcTimestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl ts_rs::TS for UtcTimestamp {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "string".to_owned()
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        Self::name(cfg)
    }
}

impl ToSql for UtcTimestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for UtcTimestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::parse(value.as_str()?).map_err(FromSqlError::other)
    }
}

/// The source of "now" for commands, audit events and the outbox.
pub trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> UtcTimestamp;
}

/// The host clock, which is the audit time source.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UtcTimestamp {
        UtcTimestamp(jiff::Timestamp::now())
    }
}

/// A clock that returns a set instant until it is moved, for deterministic tests.
#[derive(Debug)]
pub struct FixedClock(Mutex<UtcTimestamp>);

impl FixedClock {
    /// A clock stopped at `now`.
    #[must_use]
    pub fn new(now: UtcTimestamp) -> Self {
        Self(Mutex::new(now))
    }

    /// Moves the clock to `now`, forwards or backwards.
    pub fn set(&self, now: UtcTimestamp) {
        // A poisoned lock still holds a valid timestamp.
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = now;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> UtcTimestamp {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(text: &str) -> UtcTimestamp {
        UtcTimestamp::parse(text).unwrap()
    }

    #[test]
    fn canonical_form_has_nine_fraction_digits() {
        let value = UtcTimestamp(jiff::Timestamp::new(1_760_277_787, 120_000_000).unwrap());
        assert_eq!(value.to_string(), "2025-10-12T14:03:07.120000000Z");
        let whole = UtcTimestamp(jiff::Timestamp::new(0, 0).unwrap());
        assert_eq!(whole.to_string(), "1970-01-01T00:00:00.000000000Z");
        assert_eq!(ts(&value.to_string()), value);
    }

    #[test]
    fn parse_rejects_non_canonical_forms() {
        for text in [
            "2026-10-12T14:03:07Z",
            "2026-10-12T14:03:07.123Z",
            "2026-10-12T14:03:07.123456789+00:00",
            "2026-10-12T16:03:07.123456789+02:00",
            "2026-10-12t14:03:07.123456789z",
            "2026-10-12 14:03:07.123456789Z",
            "2026-10-12T14:03:07.123456789",
            "-000001-01-01T00:00:00.000000000Z",
            "",
        ] {
            assert_eq!(
                UtcTimestamp::parse(text),
                Err(ParseError::Timestamp),
                "{text}"
            );
        }
    }

    #[test]
    fn lexical_order_matches_chronological_order() {
        let earlier = ts("2026-10-12T09:59:59.999999999Z");
        let later = ts("2026-10-12T10:00:00.000000000Z");
        assert!(earlier < later);
        assert!(earlier.to_string() < later.to_string());
    }

    #[test]
    fn arithmetic_is_checked_and_saturating() {
        let start = ts("2026-10-12T10:00:00.000000000Z");
        let later = start.checked_add(Duration::from_secs(90)).unwrap();
        assert_eq!(later, ts("2026-10-12T10:01:30.000000000Z"));
        assert_eq!(
            later.saturating_duration_since(start),
            Duration::from_secs(90)
        );
        assert_eq!(start.saturating_duration_since(later), Duration::ZERO);
        let max = UtcTimestamp(jiff::Timestamp::MAX);
        assert_eq!(max.checked_add(Duration::from_secs(1)), None);
    }

    #[test]
    fn unix_seconds_drop_the_fraction() {
        assert_eq!(ts("1970-01-01T00:00:59.999999999Z").unix_seconds(), 59);
        assert_eq!(
            ts("2026-10-12T10:00:00.000000000Z").unix_seconds(),
            1_791_799_200
        );
    }

    #[test]
    fn serde_sqlite_and_typescript_use_the_string_form() {
        let value = ts("2026-10-12T10:00:00.000000001Z");
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"2026-10-12T10:00:00.000000001Z\"");
        assert_eq!(serde_json::from_str::<UtcTimestamp>(&json).unwrap(), value);
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let back: UtcTimestamp = conn.query_row("SELECT ?1", [value], |r| r.get(0)).unwrap();
        assert_eq!(back, value);
        use ts_rs::TS;
        assert_eq!(UtcTimestamp::inline(&ts_rs::Config::default()), "string");
    }

    #[test]
    fn fixed_clock_returns_the_set_instant() {
        let clock = FixedClock::new(ts("2026-10-12T10:00:00.000000000Z"));
        assert_eq!(clock.now(), ts("2026-10-12T10:00:00.000000000Z"));
        clock.set(ts("2026-10-12T11:00:00.000000000Z"));
        assert_eq!(clock.now(), ts("2026-10-12T11:00:00.000000000Z"));
    }
}
