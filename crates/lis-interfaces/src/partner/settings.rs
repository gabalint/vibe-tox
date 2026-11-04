//! Checks on connection settings, and the file names they make. Pure.

use std::path::Path;

use lis_core::required_text;

use super::error::{PartnerError, invalid};
use super::model::{PartnerConnectionSettings, RoutingRule, RuleCondition, Schedule};
use crate::validation::valid_text;

const NAME_MAX: usize = 80;
const RULES_MAX: usize = 20;
const RULE_VALUE_MAX: usize = 200;
const ONE_OF_MAX: usize = 50;
const PATTERN_MAX: usize = 120;
const FOLDER_MAX: usize = 4096;
/// Placeholders a billing file name may use.
const PLACEHOLDERS: [&str; 3] = ["{handoff}", "{date}", "{time}"];

/// The settings with their text trimmed, or the first field at fault.
pub(crate) fn valid_settings(
    settings: PartnerConnectionSettings,
) -> Result<PartnerConnectionSettings, PartnerError> {
    let name =
        required_text("settings.name", &settings.name, NAME_MAX).map_err(PartnerError::Invalid)?;
    let partner = required_text("settings.partner", &settings.partner, NAME_MAX)
        .map_err(PartnerError::Invalid)?;
    let rules = valid_rules(settings.rules)?;
    valid_schedule(&settings.schedule)?;
    let file_name_pattern = settings.file_name_pattern.trim().to_owned();
    valid_pattern(&file_name_pattern)?;
    Ok(PartnerConnectionSettings {
        name,
        partner,
        rules,
        file_name_pattern,
        ..settings
    })
}

/// A folder the engine resolved: absolute, without control characters.
pub(crate) fn valid_folder(folder: &str) -> Result<(), PartnerError> {
    if valid_text(folder, FOLDER_MAX) && Path::new(folder).is_absolute() {
        Ok(())
    } else {
        Err(invalid(
            "folder",
            "invalid_folder",
            "Choose the folder again.",
        ))
    }
}

fn valid_rules(rules: Vec<RoutingRule>) -> Result<Vec<RoutingRule>, PartnerError> {
    if rules.len() > RULES_MAX {
        return Err(invalid(
            "settings.rules",
            "too_many",
            "Use at most 20 rules.",
        ));
    }
    rules
        .into_iter()
        .enumerate()
        .map(|(index, rule)| {
            let field = format!("settings.rules.{index}.value");
            let value = required_text(&field, &rule.value, RULE_VALUE_MAX)
                .map_err(PartnerError::Invalid)?;
            if rule.condition == RuleCondition::OneOf {
                let values: Vec<&str> = value.split(',').map(str::trim).collect();
                if values.len() > ONE_OF_MAX || values.iter().any(|value| value.is_empty()) {
                    return Err(invalid(
                        &field,
                        "invalid_list",
                        "Separate up to 50 values with commas, with no empty value.",
                    ));
                }
            }
            Ok(RoutingRule { value, ..rule })
        })
        .collect()
}

fn valid_schedule(schedule: &Schedule) -> Result<(), PartnerError> {
    match schedule {
        Schedule::Immediately => Ok(()),
        Schedule::EveryMinutes { minutes } if (1..=1440).contains(minutes) => Ok(()),
        Schedule::EveryMinutes { .. } => Err(invalid(
            "settings.schedule.minutes",
            "out_of_range",
            "Use 1 to 1440 minutes.",
        )),
        Schedule::DailyAt { time } if clock_time(time).is_some() => Ok(()),
        Schedule::DailyAt { .. } => Err(invalid(
            "settings.schedule.time",
            "invalid_time",
            "Enter a time as HH:MM, from 00:00 to 23:59.",
        )),
    }
}

/// Hours and minutes of an `HH:MM` time.
pub(crate) fn clock_time(time: &str) -> Option<(i8, i8)> {
    let (hours, minutes) = time.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: i8 = hours.parse().ok()?;
    let minutes: i8 = minutes.parse().ok()?;
    ((0..24).contains(&hours) && (0..60).contains(&minutes)).then_some((hours, minutes))
}

/// A file name pattern makes a plain file name ending in `.json`, unique per
/// handoff: letters, digits, `.`, `_`, `-` and the placeholders.
fn valid_pattern(pattern: &str) -> Result<(), PartnerError> {
    let refused = |message| invalid("settings.fileNamePattern", "invalid_pattern", message);
    if pattern.is_empty() || pattern.len() > PATTERN_MAX {
        return Err(refused(
            "Enter a file name pattern of up to 120 characters.",
        ));
    }
    if !pattern.contains("{handoff}") {
        return Err(refused("Include {handoff}, so each file has its own name."));
    }
    if !pattern.ends_with(".json") || pattern.starts_with('.') {
        return Err(refused(
            "End the name with .json; do not start it with a dot.",
        ));
    }
    let plain = PLACEHOLDERS
        .iter()
        .fold(pattern.to_owned(), |text, placeholder| {
            text.replace(placeholder, "")
        });
    if !plain
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(refused(
            "Use letters, digits, '.', '_', '-' and {handoff}, {date} or {time} only.",
        ));
    }
    Ok(())
}

/// The file name `pattern` makes for handoff `number`, with `date`
/// (`YYYYMMDD`) and `time` (`HHMMSS`).
#[must_use]
pub fn file_name(pattern: &str, number: &str, date: &str, time: &str) -> String {
    pattern
        .replace("{handoff}", number)
        .replace("{date}", date)
        .replace("{time}", time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::model::{PartnerMessageType, PartnerProtocol, RuleField};

    fn settings() -> PartnerConnectionSettings {
        PartnerConnectionSettings {
            name: "  Billing - RCM ".to_owned(),
            partner: "Billing company".to_owned(),
            message_type: PartnerMessageType::BillingJson,
            protocol: PartnerProtocol::Folder,
            rules: vec![RoutingRule {
                field: RuleField::Client,
                condition: RuleCondition::OneOf,
                value: "CLINIC-A, CLINIC-B".to_owned(),
            }],
            schedule: Schedule::DailyAt {
                time: "18:30".to_owned(),
            },
            file_name_pattern: "billing-{handoff}_{date}.json".to_owned(),
        }
    }

    fn field_of(error: PartnerError) -> String {
        let fields = error.field_errors();
        assert_eq!(fields.len(), 1, "not a field error: {error}");
        fields[0].field.clone()
    }

    #[test]
    fn valid_settings_are_trimmed() {
        let valid = valid_settings(settings()).unwrap();
        assert_eq!(valid.name, "Billing - RCM");
    }

    #[test]
    fn file_name_patterns_cannot_leave_the_folder_or_repeat_names() {
        for pattern in [
            "../billing-{handoff}.json",
            "billing/{handoff}.json",
            "billing-{date}.json",
            "billing-{handoff}.hl7",
            ".billing-{handoff}.json",
            "billing {handoff}.json",
            "billing-{patient}-{handoff}.json",
        ] {
            let error = valid_settings(PartnerConnectionSettings {
                file_name_pattern: pattern.to_owned(),
                ..settings()
            })
            .unwrap_err();
            assert_eq!(field_of(error), "settings.fileNamePattern", "{pattern}");
        }
    }

    #[test]
    fn schedules_and_rule_lists_are_bounded() {
        let error = valid_settings(PartnerConnectionSettings {
            schedule: Schedule::EveryMinutes { minutes: 0 },
            ..settings()
        })
        .unwrap_err();
        assert_eq!(field_of(error), "settings.schedule.minutes");
        let error = valid_settings(PartnerConnectionSettings {
            schedule: Schedule::DailyAt {
                time: "24:00".to_owned(),
            },
            ..settings()
        })
        .unwrap_err();
        assert_eq!(field_of(error), "settings.schedule.time");
        let error = valid_settings(PartnerConnectionSettings {
            rules: vec![RoutingRule {
                field: RuleField::Client,
                condition: RuleCondition::OneOf,
                value: "CLINIC-A,,CLINIC-B".to_owned(),
            }],
            ..settings()
        })
        .unwrap_err();
        assert_eq!(field_of(error), "settings.rules.0.value");
    }

    #[test]
    fn file_names_fill_every_placeholder() {
        assert_eq!(
            file_name(
                "billing-{handoff}_{date}{time}.json",
                "12",
                "20261103",
                "101500"
            ),
            "billing-12_20261103101500.json"
        );
    }
}
