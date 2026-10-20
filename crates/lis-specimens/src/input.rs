//! Parsing of untrusted command input into checked values: text fields,
//! collection times and volumes. Pure: no database.

use std::time::Duration;

use lis_core::{Decimal, DecimalText, UtcTimestamp};
use serde::Deserialize;
use ts_rs::TS;

use crate::error::{SpecimenError, invalid};
use crate::model::{CollectionPrecision, VolumeKind};

pub(crate) const CODE_MAX: usize = 64;
pub(crate) const NAME_MAX: usize = 200;
pub(crate) const TEXT_MAX: usize = 1000;
const UNIT_MAX: usize = 16;
/// Collection offsets from UTC−12:00 to UTC+14:00, the range in civil use.
const OFFSET_MINUTES: std::ops::RangeInclusive<i32> = -720..=840;
/// Date-only collections may be at most one day ahead of the UTC date, as
/// local dates run up to 14 hours ahead of UTC.
const DATE_LEEWAY: Duration = Duration::from_secs(24 * 60 * 60);

/// [`lis_core::required_text`], as this module's error.
pub(crate) fn required_text(field: &str, value: &str, max: usize) -> Result<String, SpecimenError> {
    lis_core::required_text(field, value, max).map_err(SpecimenError::Invalid)
}

/// [`lis_core::optional_text`], as this module's error.
pub(crate) fn optional_text(
    field: &str,
    value: Option<&str>,
    max: usize,
) -> Result<Option<String>, SpecimenError> {
    lis_core::optional_text(field, value, max).map_err(SpecimenError::Invalid)
}

/// A required reason for a correction, exception or irreversible action.
pub(crate) fn reason(value: Option<&str>) -> Result<String, SpecimenError> {
    required_text("reason", value.unwrap_or_default(), TEXT_MAX)
}

/// A lab-defined code: 1–64 lowercase letters, digits, `_`, `-` or `.`.
pub(crate) fn code(field: &str, value: &str) -> Result<String, SpecimenError> {
    let value = value.trim();
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.".contains(c);
    if value.is_empty() || value.len() > CODE_MAX || !value.chars().all(allowed) {
        return Err(invalid(
            field,
            "invalid_code",
            "Use up to 64 lowercase letters, digits, dots, dashes or underscores.",
        ));
    }
    Ok(value.to_owned())
}

/// When the specimen was collected, as supplied (REQ-SPC-002).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub enum CollectionTimeInput {
    /// Local date and time with the source's UTC offset.
    #[serde(rename = "datetime", rename_all = "camelCase")]
    Datetime {
        /// `YYYY-MM-DDTHH:MM` in the collection site's local time.
        local: String,
        /// The site's offset from UTC, in minutes.
        utc_offset_minutes: i32,
    },
    /// Local date and time in a collection-site zone explicitly chosen by the operator.
    #[serde(rename = "zoned", rename_all = "camelCase")]
    Zoned {
        /// `YYYY-MM-DDTHH:MM` as recorded at the collection site.
        local: String,
        /// An IANA zone such as `America/New_York`; never inferred from the host.
        time_zone: String,
    },
    /// Date only (`YYYY-MM-DD`).
    #[serde(rename = "date")]
    Date {
        /// The collection date.
        date: String,
    },
    /// Not known.
    #[serde(rename = "unknown")]
    Unknown,
}

/// A checked collection time, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionTime {
    pub(crate) precision: CollectionPrecision,
    pub(crate) collected_at: Option<UtcTimestamp>,
    pub(crate) offset_minutes: Option<i32>,
    pub(crate) date: Option<String>,
}

/// Parses a collection time; a time after `now` is refused.
pub(crate) fn collection_time(
    input: &CollectionTimeInput,
    now: UtcTimestamp,
) -> Result<CollectionTime, SpecimenError> {
    match input {
        CollectionTimeInput::Datetime {
            local,
            utc_offset_minutes,
        } => collection_datetime(local, *utc_offset_minutes, now),
        CollectionTimeInput::Zoned { local, time_zone } => {
            let offset = collection_zone_offset(local, time_zone)?;
            collection_datetime(local, offset, now)
        }
        CollectionTimeInput::Date { date } => Ok(CollectionTime {
            precision: CollectionPrecision::Date,
            collected_at: None,
            offset_minutes: None,
            date: Some(collection_date(date, now)?),
        }),
        CollectionTimeInput::Unknown => Ok(CollectionTime {
            precision: CollectionPrecision::Unknown,
            collected_at: None,
            offset_minutes: None,
            date: None,
        }),
    }
}

fn collection_datetime(
    local: &str,
    offset_minutes: i32,
    now: UtcTimestamp,
) -> Result<CollectionTime, SpecimenError> {
    let collected_at = local_instant(local, offset_minutes)?;
    if collected_at > now {
        return Err(invalid(
            "collection.time",
            "in_future",
            "The collection time is later than now.",
        ));
    }
    Ok(CollectionTime {
        precision: CollectionPrecision::Datetime,
        collected_at: Some(collected_at),
        offset_minutes: Some(offset_minutes),
        date: None,
    })
}

fn local_civil(local: &str) -> Result<jiff::civil::DateTime, SpecimenError> {
    let bad_time = || {
        invalid(
            "collection.time",
            "invalid_time",
            "Enter the collection time as YYYY-MM-DDTHH:MM.",
        )
    };
    if local.len() != 16 || local.as_bytes().get(10) != Some(&b'T') {
        return Err(bad_time());
    }
    local.parse().map_err(|_| bad_time())
}

fn collection_zone_offset(local: &str, time_zone: &str) -> Result<i32, SpecimenError> {
    let name = required_text("collection.time.timeZone", time_zone, NAME_MAX)?;
    let zone = jiff::tz::TimeZone::get(&name).map_err(|_| {
        invalid(
            "collection.time.timeZone",
            "invalid_zone",
            "Enter a collection-site timezone such as America/New_York, or use the supplied UTC offset.",
        )
    })?;
    // REQ-SPC-002: never guess which side of a daylight-saving transition was supplied.
    let zoned = zone.to_ambiguous_zoned(local_civil(local)?).unambiguous().map_err(|_| {
        invalid(
            "collection.time",
            "ambiguous_time",
            "This collection time is ambiguous or does not exist in that timezone. Verify the time or enter its supplied UTC offset.",
        )
    })?;
    let seconds = zoned.offset().seconds();
    if seconds % 60 != 0 {
        return Err(invalid(
            "collection.time.timeZone",
            "unsupported_offset",
            "This historical timezone offset is not a whole number of minutes. Verify the collection date and supplied offset.",
        ));
    }
    Ok(seconds / 60)
}

fn local_instant(local: &str, offset_minutes: i32) -> Result<UtcTimestamp, SpecimenError> {
    let civil = local_civil(local)?;
    if !OFFSET_MINUTES.contains(&offset_minutes) {
        return Err(invalid(
            "collection.utcOffsetMinutes",
            "out_of_range",
            "Enter an offset from −720 to +840 minutes.",
        ));
    }
    let bad_time = || {
        invalid(
            "collection.time",
            "invalid_time",
            "Check the collection time and offset.",
        )
    };
    let offset = jiff::tz::Offset::from_seconds(offset_minutes * 60).map_err(|_| bad_time())?;
    let instant = offset.to_timestamp(civil).map_err(|_| bad_time())?;
    UtcTimestamp::parse(&format!("{instant:.9}")).map_err(|_| bad_time())
}

fn collection_date(text: &str, now: UtcTimestamp) -> Result<String, SpecimenError> {
    let bad_date = || {
        invalid(
            "collection.date",
            "invalid_date",
            "Enter the collection date as YYYY-MM-DD.",
        )
    };
    if text.len() != 10 {
        return Err(bad_date());
    }
    let date: jiff::civil::Date = text.parse().map_err(|_| bad_date())?;
    let latest: jiff::Timestamp = now
        .checked_add(DATE_LEEWAY)
        .ok_or_else(bad_date)?
        .to_string()
        .parse()
        .map_err(|_| bad_date())?;
    if date > latest.to_zoned(jiff::tz::TimeZone::UTC).date() {
        return Err(invalid(
            "collection.date",
            "in_future",
            "The collection date is later than today.",
        ));
    }
    Ok(date.to_string())
}

/// A checked container volume: both present with a known kind, or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Volume {
    pub(crate) amount: Option<(DecimalText, String)>,
    pub(crate) kind: VolumeKind,
}

/// Parses a volume: a positive decimal with a unit and an estimated or
/// measured kind, or no volume with kind `unknown` (REQ-SPC-003).
pub(crate) fn volume(
    field: &str,
    amount: Option<&DecimalText>,
    unit: Option<&str>,
    kind: VolumeKind,
) -> Result<Volume, SpecimenError> {
    let unit = optional_text("volumeUnit", unit, UNIT_MAX)?;
    match (amount, unit, kind) {
        (None, None, VolumeKind::Unknown) => Ok(Volume { amount: None, kind }),
        (Some(amount), Some(unit), VolumeKind::Estimated | VolumeKind::Measured) => {
            if amount.to_decimal() <= Decimal::ZERO {
                return Err(invalid(field, "not_positive", "Enter a volume above zero."));
            }
            Ok(Volume {
                amount: Some((amount.clone(), unit)),
                kind,
            })
        }
        (_, _, VolumeKind::Unknown) => Err(invalid(
            "volumeKind",
            "volume_without_kind",
            "Choose estimated or measured when you enter a volume.",
        )),
        (_, _, VolumeKind::Estimated | VolumeKind::Measured) => Err(invalid(
            field,
            "incomplete_volume",
            "Enter both a volume and its unit, or mark the volume unknown.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> UtcTimestamp {
        UtcTimestamp::parse("2026-10-12T15:00:00.000000000Z").unwrap()
    }

    fn datetime(local: &str, offset: i32) -> CollectionTimeInput {
        CollectionTimeInput::Datetime {
            local: local.to_owned(),
            utc_offset_minutes: offset,
        }
    }

    #[test]
    fn local_time_and_offset_become_utc_and_keep_the_offset() {
        let time = collection_time(&datetime("2026-10-12T08:30", -300), now()).unwrap();
        assert_eq!(time.precision, CollectionPrecision::Datetime);
        assert_eq!(
            time.collected_at.unwrap().to_string(),
            "2026-10-12T13:30:00.000000000Z"
        );
        assert_eq!(time.offset_minutes, Some(-300));
    }

    #[test]
    fn explicit_collection_zone_uses_the_offset_on_the_collection_date() {
        for (local, offset, instant) in [
            ("2026-01-12T08:30", -300, "2026-01-12T13:30:00.000000000Z"),
            ("2026-07-12T08:30", -240, "2026-07-12T12:30:00.000000000Z"),
        ] {
            let input = CollectionTimeInput::Zoned {
                local: local.to_owned(),
                time_zone: "America/New_York".to_owned(),
            };
            let time = collection_time(&input, now()).unwrap();
            assert_eq!(time.offset_minutes, Some(offset));
            assert_eq!(time.collected_at.unwrap().to_string(), instant);
            assert_eq!(
                time,
                collection_time(&datetime(local, offset), now()).unwrap()
            );
        }
    }

    #[test]
    fn collection_zone_never_guesses_at_dst_gaps_or_folds() {
        for local in ["2026-03-08T02:30", "2025-11-02T01:30"] {
            let input = CollectionTimeInput::Zoned {
                local: local.to_owned(),
                time_zone: "America/New_York".to_owned(),
            };
            let SpecimenError::Invalid(field) = collection_time(&input, now()).unwrap_err() else {
                panic!("expected a field error");
            };
            assert_eq!(field.field, "collection.time");
            assert_eq!(field.code, "ambiguous_time");
        }
        // Both supplied offsets remain representable for the repeated clock time.
        let first = collection_time(&datetime("2025-11-02T01:30", -240), now()).unwrap();
        let second = collection_time(&datetime("2025-11-02T01:30", -300), now()).unwrap();
        assert_ne!(first.collected_at, second.collected_at);
    }

    #[test]
    fn collection_zone_refuses_invalid_zones_malformed_times_and_future_instants() {
        for (local, zone) in [
            ("2026-10-12T08:30", ""),
            ("2026-10-12T08:30", "America/Not_A_Zone"),
            ("2026-13-12T08:30", "America/New_York"),
            ("2026-10-12T08:30:00", "America/New_York"),
            ("2026-10-12T12:00", "America/New_York"),
        ] {
            let input = CollectionTimeInput::Zoned {
                local: local.to_owned(),
                time_zone: zone.to_owned(),
            };
            assert!(collection_time(&input, now()).is_err(), "{local} {zone}");
        }
        let oversized = CollectionTimeInput::Zoned {
            local: "2026-10-12T08:30".to_owned(),
            time_zone: "x".repeat(NAME_MAX + 1),
        };
        assert!(collection_time(&oversized, now()).is_err());
    }

    #[test]
    fn malformed_out_of_range_or_future_times_are_refused() {
        for (local, offset) in [
            ("2026-10-12 08:30", 0),
            ("2026-10-12T08:30:00", 0),
            ("2026-13-12T08:30", 0),
            ("2026-10-12T08:30", 900),
            ("2026-10-12T16:00", 0),
        ] {
            assert!(
                collection_time(&datetime(local, offset), now()).is_err(),
                "{local} {offset}"
            );
        }
    }

    #[test]
    fn date_only_and_unknown_stay_explicit() {
        let date = CollectionTimeInput::Date {
            date: "2026-10-13".to_owned(),
        };
        let time = collection_time(&date, now()).unwrap();
        assert_eq!(
            (time.precision, time.date.as_deref(), time.collected_at),
            (CollectionPrecision::Date, Some("2026-10-13"), None)
        );
        let later = CollectionTimeInput::Date {
            date: "2026-10-14".to_owned(),
        };
        assert!(collection_time(&later, now()).is_err());
        let unknown = collection_time(&CollectionTimeInput::Unknown, now()).unwrap();
        assert_eq!(unknown.precision, CollectionPrecision::Unknown);
        assert_eq!(unknown.collected_at, None);
    }

    #[test]
    fn volumes_need_amount_unit_and_kind_together() {
        let amount = DecimalText::parse("30.0").unwrap();
        let ok = volume("volume", Some(&amount), Some("mL"), VolumeKind::Measured).unwrap();
        assert_eq!(ok.amount, Some((amount.clone(), "mL".to_owned())));
        assert!(volume("volume", None, None, VolumeKind::Unknown).is_ok());
        assert!(volume("volume", Some(&amount), None, VolumeKind::Measured).is_err());
        assert!(volume("volume", Some(&amount), Some("mL"), VolumeKind::Unknown).is_err());
        assert!(volume("volume", None, None, VolumeKind::Estimated).is_err());
        let zero = DecimalText::parse("0").unwrap();
        assert!(volume("volume", Some(&zero), Some("mL"), VolumeKind::Measured).is_err());
        let negative = DecimalText::parse("-1").unwrap();
        assert!(volume("volume", Some(&negative), Some("mL"), VolumeKind::Measured).is_err());
    }

    #[test]
    fn unknown_fields_in_a_collection_time_are_refused() {
        let extra = serde_json::json!({ "kind": "date", "date": "2026-10-12", "time": "08:00" });
        assert!(serde_json::from_value::<CollectionTimeInput>(extra).is_err());
        let datetime = serde_json::json!({
            "kind": "datetime",
            "local": "2026-10-12T08:00",
            "utcOffsetMinutes": -300
        });
        assert!(serde_json::from_value::<CollectionTimeInput>(datetime).is_ok());
        let mut zoned = serde_json::json!({
            "kind": "zoned", "local": "2026-10-12T08:00", "timeZone": "America/New_York"
        });
        assert!(serde_json::from_value::<CollectionTimeInput>(zoned.clone()).is_ok());
        zoned["utcOffsetMinutes"] = serde_json::json!(-300);
        assert!(serde_json::from_value::<CollectionTimeInput>(zoned).is_err());
    }

    #[test]
    fn codes_are_machine_tokens() {
        assert_eq!(code("code", " temp_in_range ").unwrap(), "temp_in_range");
        assert!(code("code", "Temp In Range").is_err());
        assert!(code("code", "").is_err());
    }
}
