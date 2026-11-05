//! Checks on connection settings, and the file names they make. Pure.

use std::path::Path;

use lis_core::required_text;

use super::error::{PartnerError, invalid};
use super::intake_settings::valid_intake;
use super::mllp_settings::valid_mllp;
use super::model::{
    PartnerConnectionSettings, PartnerMessageType, RoutingRule, RuleCondition, Schedule,
};
use super::sftp_settings::valid_sftp;
use crate::validation::valid_text;

const NAME_MAX: usize = 80;
const RULES_MAX: usize = 20;
const RULE_VALUE_MAX: usize = 200;
const ONE_OF_MAX: usize = 50;
const PATTERN_MAX: usize = 120;
const FOLDER_MAX: usize = 4096;

/// What a file name pattern of one message type holds: placeholders it must use,
/// so each file has its own name, ones it may use, and its extension.
struct PatternRules {
    required: &'static [&'static str],
    optional: &'static [&'static str],
    extension: &'static str,
}

/// The file name rules of an outgoing type; `None` for an incoming one, which writes no file.
const fn pattern_rules(message_type: PartnerMessageType) -> Option<PatternRules> {
    match message_type {
        PartnerMessageType::BillingJson => Some(PatternRules {
            required: &["{handoff}"],
            optional: &["{date}", "{time}"],
            extension: ".json",
        }),
        PartnerMessageType::ReportPdf => Some(PatternRules {
            required: &["{accession}", "{revision}"],
            optional: &["{date}", "{time}"],
            extension: ".pdf",
        }),
        PartnerMessageType::Hl7Oru => Some(PatternRules {
            required: &["{accession}", "{revision}"],
            optional: &["{date}", "{time}"],
            extension: ".hl7",
        }),
        PartnerMessageType::Hl7Orders => None,
    }
}

/// The settings with their text trimmed, or the first field at fault. An incoming
/// connection routes, schedules and names nothing: it keeps no rules, writes at once and
/// has no file name pattern. An MLLP connection sends each message as it is, unnamed.
pub(crate) fn valid_settings(
    settings: PartnerConnectionSettings,
) -> Result<PartnerConnectionSettings, PartnerError> {
    let name =
        required_text("settings.name", &settings.name, NAME_MAX).map_err(PartnerError::Invalid)?;
    let partner = required_text("settings.partner", &settings.partner, NAME_MAX)
        .map_err(PartnerError::Invalid)?;
    let sftp = valid_sftp(settings.protocol, settings.sftp)?;
    let mllp = valid_mllp(settings.message_type, settings.protocol, settings.mllp)?;
    let intake = valid_intake(settings.message_type, settings.protocol, settings.intake)?;
    let Some(pattern) = pattern_rules(settings.message_type) else {
        return Ok(PartnerConnectionSettings {
            name,
            partner,
            rules: Vec::new(),
            schedule: Schedule::Immediately,
            file_name_pattern: String::new(),
            sftp,
            intake,
            mllp,
            ..settings
        });
    };
    let rules = valid_rules(settings.rules)?;
    valid_schedule(&settings.schedule)?;
    let file_name_pattern = if mllp.is_some() {
        String::new()
    } else {
        let file_name_pattern = settings.file_name_pattern.trim().to_owned();
        valid_pattern(&file_name_pattern, &pattern)?;
        file_name_pattern
    };
    Ok(PartnerConnectionSettings {
        name,
        partner,
        rules,
        file_name_pattern,
        sftp,
        intake,
        mllp,
        ..settings
    })
}

/// A folder the engine resolved: absolute, without control characters.
pub(crate) fn valid_folder(folder: &str) -> Result<(), PartnerError> {
    valid_folder_at("folder", folder)
}

/// [`valid_folder`], for the form field `field`.
pub(crate) fn valid_folder_at(field: &str, folder: &str) -> Result<(), PartnerError> {
    if valid_text(folder, FOLDER_MAX) && Path::new(folder).is_absolute() {
        Ok(())
    } else {
        Err(invalid(field, "invalid_folder", "Choose the folder again."))
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

/// A file name pattern makes a plain file name with the type's extension, unique
/// per message: letters, digits, `.`, `_`, `-` and the type's placeholders.
fn valid_pattern(pattern: &str, rules: &PatternRules) -> Result<(), PartnerError> {
    let refused = |message: &str| invalid("settings.fileNamePattern", "invalid_pattern", message);
    if pattern.is_empty() || pattern.len() > PATTERN_MAX {
        return Err(refused(
            "Enter a file name pattern of up to 120 characters.",
        ));
    }
    if !rules
        .required
        .iter()
        .all(|placeholder| pattern.contains(placeholder))
    {
        return Err(refused(&format!(
            "Include {}, so each file has its own name.",
            rules.required.join(" and ")
        )));
    }
    if !pattern.ends_with(rules.extension) || pattern.starts_with('.') {
        return Err(refused(&format!(
            "End the name with {}; do not start it with a dot.",
            rules.extension
        )));
    }
    let plain = rules
        .required
        .iter()
        .chain(rules.optional)
        .fold(pattern.to_owned(), |text, placeholder| {
            text.replace(placeholder, "")
        });
    if !plain.bytes().all(plain_byte) {
        let placeholders: Vec<&str> = rules
            .required
            .iter()
            .chain(rules.optional)
            .copied()
            .collect();
        return Err(refused(&format!(
            "Use letters, digits, '.', '_', '-' and {} only.",
            placeholders.join(", ")
        )));
    }
    Ok(())
}

fn plain_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

/// The values a file name pattern's placeholders take.
pub(crate) struct FileNameParts<'a> {
    /// `{handoff}` or `{accession}`: the handoff number, or the report number.
    pub(crate) record: &'a str,
    /// `{revision}`: the report's revision number.
    pub(crate) revision: &'a str,
    /// `{date}`: `YYYYMMDD`.
    pub(crate) date: &'a str,
    /// `{time}`: `HHMMSS`.
    pub(crate) time: &'a str,
}

/// The file name `pattern` makes with `parts`. A character a file name may not hold
/// becomes `_`, so a value never leaves the folder.
#[must_use]
pub(crate) fn file_name(pattern: &str, parts: &FileNameParts<'_>) -> String {
    let plain = |value: &str| -> String {
        value
            .bytes()
            .map(|byte| {
                if plain_byte(byte) {
                    char::from(byte)
                } else {
                    '_'
                }
            })
            .collect()
    };
    let record = plain(parts.record);
    pattern
        .replace("{handoff}", &record)
        .replace("{accession}", &record)
        .replace("{revision}", &plain(parts.revision))
        .replace("{date}", &plain(parts.date))
        .replace("{time}", &plain(parts.time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::model::{PartnerProtocol, RuleField};

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
            sftp: None,
            intake: None,
            mllp: None,
        }
    }

    #[test]
    fn an_mllp_connection_keeps_its_rules_and_names_no_file() {
        use crate::partner::model::{MllpAcknowledgement, MllpPartner};
        let valid = valid_settings(PartnerConnectionSettings {
            message_type: PartnerMessageType::Hl7Oru,
            protocol: PartnerProtocol::MllpConnect,
            file_name_pattern: "not a pattern".to_owned(),
            mllp: Some(MllpPartner {
                address: "10.20.0.15".to_owned(),
                port: 6661,
                acknowledgement: MllpAcknowledgement::Application,
                ack_timeout_seconds: 30,
            }),
            ..settings()
        })
        .unwrap();
        assert_eq!(valid.file_name_pattern, "");
        assert_eq!(valid.rules.len(), 1);
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
    fn report_patterns_need_the_report_and_its_revision() {
        let report = |pattern: &str| {
            valid_settings(PartnerConnectionSettings {
                message_type: PartnerMessageType::ReportPdf,
                file_name_pattern: pattern.to_owned(),
                ..settings()
            })
        };
        assert!(report("{accession}_r{revision}_{date}.pdf").is_ok());
        for pattern in [
            "{accession}_{date}.pdf",
            "{accession}_r{revision}.json",
            "{handoff}_r{revision}.pdf",
        ] {
            assert_eq!(
                field_of(report(pattern).unwrap_err()),
                "settings.fileNamePattern",
                "{pattern}"
            );
        }
    }

    #[test]
    fn file_names_fill_every_placeholder() {
        let parts = FileNameParts {
            record: "12",
            revision: "",
            date: "20261103",
            time: "101500",
        };
        assert_eq!(
            file_name("billing-{handoff}_{date}{time}.json", &parts),
            "billing-12_20261103101500.json"
        );
        let parts = FileNameParts {
            record: "A26000005-U/1",
            revision: "2-withdrawn",
            ..parts
        };
        assert_eq!(
            file_name("{accession}_r{revision}_{date}.pdf", &parts),
            "A26000005-U_1_r2-withdrawn_20261103.pdf"
        );
    }
}
