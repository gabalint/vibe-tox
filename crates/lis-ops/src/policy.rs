//! The one home of the operations defaults, setting ranges and fixed limits (M4 design brief
//! §3, §6.1 and §8). The laboratory changes the settings through versioned rows; until it
//! does, these defaults apply (laboratory questions L2–L6, L8). Fixed limits are engineering
//! limits, not laboratory policy.

use std::ops::RangeInclusive;

use lis_core::FieldError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The laboratory-set operations values: backup schedule, retention and health thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OperationsSettings {
    /// Minutes between scheduled backups while the app is open (NFR-30).
    pub backup_interval_minutes: u32,
    /// Newest sets kept per target.
    pub keep_recent: u32,
    /// Days for which the newest set of each UTC day is kept.
    pub keep_daily: u32,
    /// Weeks for which the newest set of each ISO week is kept.
    pub keep_weekly: u32,
    /// Backup age that raises a warning.
    pub backup_warn_minutes: u32,
    /// Backup age that raises an alert.
    pub backup_alert_minutes: u32,
    /// Off-host copy age that raises a warning.
    pub offhost_warn_hours: u32,
    /// Offline (disconnected removable) copy age that raises a warning (NFR-20).
    pub offline_warn_days: u32,
    /// Restore-test age that raises a warning.
    pub restore_test_warn_days: u32,
    /// Recovery-kit storage confirmation age that raises a warning.
    pub kit_confirm_warn_days: u32,
    /// Anchor lag that raises a warning.
    pub anchor_warn_minutes: u32,
    /// Anchor lag that raises an incident alert and work item.
    pub anchor_incident_hours: u32,
    /// Free space on the data folder's disk below which a warning is raised.
    pub disk_free_warn_gib: u32,
    /// Days before an operating system's end of support that raise a warning.
    pub os_support_warn_days: u32,
    /// A wall-clock jump larger than this is recorded.
    pub clock_jump_seconds: u32,
}

/// The defaults that apply until the laboratory sets its own values. Since 2026-10-13 the
/// backup is hourly: every copy is a full copy, so the 15-minute recovery point (NFR-04,
/// NFR-30) waits for incremental copies (backup incremental design brief). The warning
/// age is two missed intervals.
pub const DEFAULT_SETTINGS: OperationsSettings = OperationsSettings {
    backup_interval_minutes: 60,
    keep_recent: 6,
    keep_daily: 7,
    keep_weekly: 4,
    backup_warn_minutes: 2 * 60,
    backup_alert_minutes: 4 * 60,
    offhost_warn_hours: 24,
    offline_warn_days: 7,
    restore_test_warn_days: 90,
    kit_confirm_warn_days: 90,
    anchor_warn_minutes: 30,
    anchor_incident_hours: 24,
    disk_free_warn_gib: 10,
    os_support_warn_days: 90,
    clock_jump_seconds: 60,
};

/// Allowed backup intervals, in minutes.
pub const BACKUP_INTERVAL_MINUTES: RangeInclusive<u32> = 5..=1440;
/// Allowed numbers of recent sets kept per target.
pub const KEEP_RECENT: RangeInclusive<u32> = 2..=96;
/// Allowed numbers of daily sets kept per target.
pub const KEEP_DAILY: RangeInclusive<u32> = 0..=90;
/// Allowed numbers of weekly sets kept per target.
pub const KEEP_WEEKLY: RangeInclusive<u32> = 0..=104;

/// Allowed backup ages that raise a warning, in minutes.
pub const BACKUP_WARN_MINUTES: RangeInclusive<u32> = 10..=1440;
/// Allowed backup ages that raise an alert, in minutes (more than the warning age).
pub const BACKUP_ALERT_MINUTES: RangeInclusive<u32> = 30..=10_080;
/// Allowed off-host copy ages that raise a warning, in hours.
pub const OFFHOST_WARN_HOURS: RangeInclusive<u32> = 1..=168;
/// Allowed offline copy ages that raise a warning, in days.
pub const OFFLINE_WARN_DAYS: RangeInclusive<u32> = 1..=90;
/// Allowed restore-test ages that raise a warning, in days.
pub const RESTORE_TEST_WARN_DAYS: RangeInclusive<u32> = 7..=365;
/// Allowed recovery-kit confirmation ages that raise a warning, in days.
pub const KIT_CONFIRM_WARN_DAYS: RangeInclusive<u32> = 7..=365;
/// Allowed anchor lags that raise a warning, in minutes.
pub const ANCHOR_WARN_MINUTES: RangeInclusive<u32> = 10..=1440;
/// Allowed anchor lags that raise an incident alert, in hours (more than the warning lag).
pub const ANCHOR_INCIDENT_HOURS: RangeInclusive<u32> = 1..=168;
/// Allowed free-space warning levels, in GiB.
pub const DISK_FREE_WARN_GIB: RangeInclusive<u32> = 1..=1024;
/// Allowed warning periods before an operating system's end of support, in days.
pub const OS_SUPPORT_WARN_DAYS: RangeInclusive<u32> = 7..=365;
/// Allowed clock-jump recording thresholds, in seconds.
pub const CLOCK_JUMP_SECONDS: RangeInclusive<u32> = 10..=3600;

/// One setting: its input field name, its audit code, its value and its allowed range.
pub(crate) struct SettingField {
    /// The camelCase input and view field name.
    pub(crate) field: &'static str,
    /// The snake_case code, as in the audit detail `changed`.
    pub(crate) code: &'static str,
    /// The value.
    pub(crate) value: u32,
    /// The allowed range.
    pub(crate) range: RangeInclusive<u32>,
}

impl OperationsSettings {
    /// Every value outside its range, a backup warning age not above the interval, and
    /// every alert level not above its warning level, as field errors named after the
    /// camelCase input fields.
    #[must_use]
    pub fn problems(&self) -> Vec<FieldError> {
        let mut problems: Vec<FieldError> = self
            .fields()
            .into_iter()
            .filter(|setting| !setting.range.contains(&setting.value))
            .map(|setting| {
                field_error(
                    setting.field,
                    "out_of_range",
                    format!(
                        "Enter a value from {} to {}.",
                        setting.range.start(),
                        setting.range.end()
                    ),
                )
            })
            .collect();
        if self.backup_warn_minutes <= self.backup_interval_minutes {
            problems.push(field_error(
                "backupWarnMinutes",
                "not_above_interval",
                "The warning age must be above the backup interval.".to_owned(),
            ));
        }
        if self.backup_alert_minutes <= self.backup_warn_minutes {
            problems.push(above_warning("backupAlertMinutes"));
        }
        if u64::from(self.anchor_incident_hours) * 60 <= u64::from(self.anchor_warn_minutes) {
            problems.push(above_warning("anchorIncidentHours"));
        }
        problems
    }

    /// Every setting with its names and range, in declaration order.
    pub(crate) fn fields(&self) -> [SettingField; 15] {
        SETTING_FIELDS.map(|(field, code, value, range)| SettingField {
            field,
            code,
            value: value(self),
            range,
        })
    }
}

type Getter = fn(&OperationsSettings) -> u32;

/// Input field name, audit code, value and range of every setting.
const SETTING_FIELDS: [(&str, &str, Getter, RangeInclusive<u32>); 15] = [
    (
        "backupIntervalMinutes",
        "backup_interval_minutes",
        |settings| settings.backup_interval_minutes,
        BACKUP_INTERVAL_MINUTES,
    ),
    (
        "keepRecent",
        "keep_recent",
        |settings| settings.keep_recent,
        KEEP_RECENT,
    ),
    (
        "keepDaily",
        "keep_daily",
        |settings| settings.keep_daily,
        KEEP_DAILY,
    ),
    (
        "keepWeekly",
        "keep_weekly",
        |settings| settings.keep_weekly,
        KEEP_WEEKLY,
    ),
    (
        "backupWarnMinutes",
        "backup_warn_minutes",
        |settings| settings.backup_warn_minutes,
        BACKUP_WARN_MINUTES,
    ),
    (
        "backupAlertMinutes",
        "backup_alert_minutes",
        |settings| settings.backup_alert_minutes,
        BACKUP_ALERT_MINUTES,
    ),
    (
        "offhostWarnHours",
        "offhost_warn_hours",
        |settings| settings.offhost_warn_hours,
        OFFHOST_WARN_HOURS,
    ),
    (
        "offlineWarnDays",
        "offline_warn_days",
        |settings| settings.offline_warn_days,
        OFFLINE_WARN_DAYS,
    ),
    (
        "restoreTestWarnDays",
        "restore_test_warn_days",
        |settings| settings.restore_test_warn_days,
        RESTORE_TEST_WARN_DAYS,
    ),
    (
        "kitConfirmWarnDays",
        "kit_confirm_warn_days",
        |settings| settings.kit_confirm_warn_days,
        KIT_CONFIRM_WARN_DAYS,
    ),
    (
        "anchorWarnMinutes",
        "anchor_warn_minutes",
        |settings| settings.anchor_warn_minutes,
        ANCHOR_WARN_MINUTES,
    ),
    (
        "anchorIncidentHours",
        "anchor_incident_hours",
        |settings| settings.anchor_incident_hours,
        ANCHOR_INCIDENT_HOURS,
    ),
    (
        "diskFreeWarnGib",
        "disk_free_warn_gib",
        |settings| settings.disk_free_warn_gib,
        DISK_FREE_WARN_GIB,
    ),
    (
        "osSupportWarnDays",
        "os_support_warn_days",
        |settings| settings.os_support_warn_days,
        OS_SUPPORT_WARN_DAYS,
    ),
    (
        "clockJumpSeconds",
        "clock_jump_seconds",
        |settings| settings.clock_jump_seconds,
        CLOCK_JUMP_SECONDS,
    ),
];

fn above_warning(field: &str) -> FieldError {
    field_error(
        field,
        "not_above_warning",
        "The alert level must be above the warning level.".to_owned(),
    )
}

fn field_error(field: &str, code: &str, message: String) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message,
    }
}

/// Seals and anchor exports run at least this often while the app is open, and after
/// every backup, whatever the backup interval (NFR-18, M4 design brief §4).
pub const SEAL_INTERVAL_MINUTES: u32 = 15;
/// The longest note, reason or medium description accepted, in characters.
pub(crate) const NOTE_MAX_CHARS: usize = 500;
/// A pre-update backup must have finished at most this many hours before the approval.
pub const PRE_UPDATE_MAX_AGE_HOURS: u32 = 24;
/// How long a restore session keeps the entered installation key in memory.
pub const RESTORE_SESSION_MINUTES: u32 = 30;
/// Accession numbers skipped after a restore, above the highest restored number.
pub const RESTORE_ACCESSION_SKIP: u32 = 1_000;
/// Container codes skipped after a restore, above the highest restored code.
pub const RESTORE_CONTAINER_SKIP: u32 = 5_000;
/// A stop longer than this between two runs is a listening gap.
pub const LISTENING_GAP_SECONDS: u32 = 60;
/// The longest validity of a health-check attestation.
pub const ATTESTATION_MAX_DAYS: u32 = 90;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid() {
        assert_eq!(DEFAULT_SETTINGS.problems(), []);
    }

    #[test]
    fn every_range_is_inclusive_at_both_ends() {
        for (index, setting) in DEFAULT_SETTINGS.fields().into_iter().enumerate() {
            let (field, range) = (setting.field, setting.range);
            for (value, valid) in [
                (*range.start(), true),
                (*range.end(), true),
                (range.start().wrapping_sub(1), false),
                (range.end() + 1, false),
            ] {
                let mut values = DEFAULT_SETTINGS.fields().map(|setting| setting.value);
                values[index] = value;
                let problems = from_values(values).problems();
                let flagged = problems
                    .iter()
                    .any(|p| p.field == field && p.code == "out_of_range");
                assert_eq!(flagged, !valid, "{field} = {value}");
            }
        }
    }

    #[test]
    fn the_backup_warning_age_must_be_above_the_interval() {
        for (warn, flagged) in [(60, true), (61, false)] {
            let settings = OperationsSettings {
                backup_interval_minutes: 60,
                backup_warn_minutes: warn,
                ..DEFAULT_SETTINGS
            };
            let codes: Vec<(String, String)> = settings
                .problems()
                .into_iter()
                .map(|p| (p.field, p.code))
                .collect();
            let expected = [("backupWarnMinutes".into(), "not_above_interval".into())];
            assert_eq!(codes, if flagged { &expected[..] } else { &[] }, "{warn}");
        }
    }

    #[test]
    fn alert_levels_must_be_above_warning_levels() {
        let equal = OperationsSettings {
            backup_warn_minutes: 180,
            backup_alert_minutes: 180,
            anchor_warn_minutes: 120,
            anchor_incident_hours: 2,
            ..DEFAULT_SETTINGS
        };
        let codes: Vec<(String, String)> = equal
            .problems()
            .into_iter()
            .map(|p| (p.field, p.code))
            .collect();
        assert_eq!(
            codes,
            [
                ("backupAlertMinutes".into(), "not_above_warning".into()),
                ("anchorIncidentHours".into(), "not_above_warning".into())
            ]
        );
    }

    fn from_values(v: [u32; 15]) -> OperationsSettings {
        OperationsSettings {
            backup_interval_minutes: v[0],
            keep_recent: v[1],
            keep_daily: v[2],
            keep_weekly: v[3],
            backup_warn_minutes: v[4],
            backup_alert_minutes: v[5],
            offhost_warn_hours: v[6],
            offline_warn_days: v[7],
            restore_test_warn_days: v[8],
            kit_confirm_warn_days: v[9],
            anchor_warn_minutes: v[10],
            anchor_incident_hours: v[11],
            disk_free_warn_gib: v[12],
            os_support_warn_days: v[13],
            clock_jump_seconds: v[14],
        }
    }
}
