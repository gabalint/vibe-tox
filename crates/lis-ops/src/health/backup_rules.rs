//! The §6.1 rules about backups, the recovery kit and audit anchors (§3.5, §4).

use std::time::Duration;

use lis_core::UtcTimestamp;

use super::checks::HealthState;
use super::evaluate::{Finding, age_text, days, hours, minutes};
use super::inputs::HealthInputs;
use crate::policy::OperationsSettings;

/// Warning at or above `warn`, alert at or above `alert` when given.
fn by_age(what: &str, age: Duration, warn: Duration, alert: Option<Duration>) -> Finding {
    let state = if alert.is_some_and(|alert| age >= alert) {
        HealthState::Alert
    } else if age >= warn {
        HealthState::Warning
    } else {
        HealthState::Ok
    };
    let threshold = match alert {
        Some(alert) => format!(
            "warning at {}, alert at {}",
            age_text(warn),
            age_text(alert)
        ),
        None => format!("warning at {}", age_text(warn)),
    };
    Finding::new(state, "too_old", format!("{what}: {}.", age_text(age)))
        .value(age_text(age))
        .threshold(threshold)
}

pub(super) fn backup_configured(inputs: &HealthInputs) -> Finding {
    match inputs.backup.active_targets {
        0 => Finding::new(HealthState::Alert, "no_target", "No active backup target."),
        count => Finding::ok(format!("{count} active backup target(s).")).value(count.to_string()),
    }
}

pub(super) fn backup_age(inputs: &HealthInputs, settings: &OperationsSettings) -> Finding {
    match inputs.backup.backup_age {
        None => Finding::new(HealthState::Alert, "never", "No verified backup yet."),
        Some(age) => by_age(
            "Oldest change not in a verified backup",
            age,
            minutes(settings.backup_warn_minutes),
            Some(minutes(settings.backup_alert_minutes)),
        ),
    }
}

pub(super) fn offhost_copy_age(inputs: &HealthInputs, settings: &OperationsSettings) -> Finding {
    match inputs.backup.offhost_age {
        None => Finding::new(HealthState::Warning, "none", "No verified off-host copy."),
        Some(age) => by_age(
            "Oldest change not in a verified off-host copy",
            age,
            hours(settings.offhost_warn_hours),
            None,
        ),
    }
}

pub(super) fn offline_copy_age(inputs: &HealthInputs, settings: &OperationsSettings) -> Finding {
    if inputs.backup.removable_targets == 0 {
        // NFR-20: a rotated removable target is the offline copy.
        return Finding::new(
            HealthState::Warning,
            "no_removable_target",
            "No removable target for an offline copy.",
        );
    }
    match inputs.backup.offline_age {
        None => Finding::new(
            HealthState::Warning,
            "none",
            "No disconnected removable target holds a verified copy.",
        ),
        Some(age) => by_age(
            "Newest offline copy",
            age,
            days(settings.offline_warn_days),
            None,
        ),
    }
}

pub(super) fn restore_test_age(
    inputs: &HealthInputs,
    settings: &OperationsSettings,
    now: UtcTimestamp,
) -> Finding {
    match inputs.backup.last_passed_restore_test_at {
        None => Finding::new(
            HealthState::Alert,
            "never",
            "No restore test has passed yet.",
        ),
        Some(at) => by_age(
            "Last passed restore test",
            now.saturating_duration_since(at),
            days(settings.restore_test_warn_days),
            None,
        ),
    }
}

pub(super) fn recovery_kit(
    inputs: &HealthInputs,
    settings: &OperationsSettings,
    now: UtcTimestamp,
) -> Finding {
    let kit = &inputs.backup.kit;
    if kit.last_export_at.is_none() {
        return Finding::new(
            HealthState::Alert,
            "never_exported",
            "The recovery kit was never exported.",
        );
    }
    if kit.outdated {
        return Finding::new(
            HealthState::Warning,
            "outdated",
            "The key or the backup locations changed since the last kit.",
        );
    }
    match kit.last_confirmation_at {
        None => Finding::new(
            HealthState::Warning,
            "not_confirmed",
            "Storage of the latest kit is not confirmed.",
        ),
        Some(at) => by_age(
            "Kit storage last confirmed",
            now.saturating_duration_since(at),
            days(settings.kit_confirm_warn_days),
            None,
        ),
    }
}

pub(super) fn anchor_lag(inputs: &HealthInputs, settings: &OperationsSettings) -> Finding {
    if inputs.anchors.offhost_targets == 0 {
        return Finding::new(
            HealthState::Alert,
            "no_offhost_target",
            "No off-host target receives audit anchors.",
        );
    }
    match inputs.anchors.lag {
        None => Finding::ok("Every audit event is anchored off-host."),
        Some(lag) => by_age(
            "Oldest audit event not anchored off-host",
            lag,
            minutes(settings.anchor_warn_minutes),
            Some(hours(settings.anchor_incident_hours)),
        ),
    }
}

/// Failed runs or copies in a row that raise the `backup_failures` alert (§3.5).
const FAILURES_ALERT: u32 = 3;

/// Warning on any failed run or copy since the last success, alert at 3 in a row
/// (§3.5); a run counts once even when every copy failed.
pub(super) fn backup_failures(inputs: &HealthInputs) -> Finding {
    let backup = &inputs.backup;
    let (count, summary) = if backup.failed_runs_in_a_row >= backup.failed_copies_in_a_row {
        (backup.failed_runs_in_a_row, "backup run(s) in a row failed")
    } else {
        (
            backup.failed_copies_in_a_row,
            "copies in a row to one backup target failed",
        )
    };
    let state = match count {
        0 => return Finding::ok("No backup run or copy failed since the last success."),
        count if count >= FAILURES_ALERT => HealthState::Alert,
        _ => HealthState::Warning,
    };
    Finding::new(state, "failed_in_a_row", format!("{count} {summary}."))
        .value(count.to_string())
        .threshold(format!("warning at 1, alert at {FAILURES_ALERT}"))
}

/// Alert while the newest checked snapshot failed `PRAGMA integrity_check` (§3.1 step
/// 4): the live database may be damaged. The next intact snapshot clears it.
pub(super) fn database_integrity(inputs: &HealthInputs) -> Finding {
    match inputs.backup.snapshot_intact {
        None => Finding::new(
            HealthState::Unknown,
            "not_checked",
            "No backup snapshot has been checked yet.",
        ),
        Some(true) => Finding::ok("The newest backup snapshot passed its integrity check."),
        Some(false) => Finding::new(
            HealthState::Alert,
            "snapshot_not_intact",
            "The newest backup snapshot failed its integrity check: the database may be damaged.",
        ),
    }
}

pub(super) fn artifact_store(inputs: &HealthInputs) -> Finding {
    match inputs.backup.artifact_problems {
        0 => Finding::ok("The last backup read every stored file."),
        count => Finding::new(
            HealthState::Alert,
            "problems",
            format!("{count} stored file(s) missing or damaged."),
        )
        .value(count.to_string()),
    }
}
