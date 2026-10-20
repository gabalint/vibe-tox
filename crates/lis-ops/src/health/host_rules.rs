//! The §6.1 rules about the host: disk space and encryption, operating-system support,
//! web engine, listeners and sleep prevention, unclean stops, clock and UPS.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use lis_core::UtcTimestamp;

use super::checks::{AttestationCode, HealthState};
use super::evaluate::{Finding, hours, unknown_or_attested, valid_attestation};
use super::inputs::{HealthInputs, KeepAwakeState};
use crate::host::{self, DiskEncryption, SupportStatus};
use crate::policy::OperationsSettings;

const GIB: u64 = 1 << 30;
/// How long a backward clock jump keeps the clock check in alert.
const CLOCK_ALERT_HOURS: u32 = 24;

fn gib_text(bytes: u64) -> String {
    format!("{} GiB", bytes / GIB)
}

/// Warning below max(the lab's GiB, 3 × the database); alert below 1.2 × the database.
pub(super) fn disk_free(inputs: &HealthInputs, settings: &OperationsSettings) -> Finding {
    let disk = &inputs.disk;
    let Some(available) = disk.available_bytes else {
        return Finding::new(
            HealthState::Unknown,
            "not_read",
            "Free space could not be read.",
        );
    };
    let alert_below = disk.database_bytes.saturating_mul(6) / 5;
    let warn_below =
        (u64::from(settings.disk_free_warn_gib) * GIB).max(disk.database_bytes.saturating_mul(3));
    let state = if available < alert_below {
        HealthState::Alert
    } else if available < warn_below {
        HealthState::Warning
    } else {
        HealthState::Ok
    };
    let projection = disk
        .growth_bytes_per_day
        .filter(|growth| *growth > 0)
        .map(|growth| {
            format!(
                " At the recent growth, full in about {} days.",
                available / growth
            )
        })
        .unwrap_or_default();
    Finding::new(
        state,
        "low_space",
        format!("{} free.{projection}", gib_text(available)),
    )
    .value(gib_text(available))
    .threshold(format!(
        "warning below {}, alert below {}",
        gib_text(warn_below),
        gib_text(alert_below)
    ))
}

pub(super) fn disk_encryption(inputs: &HealthInputs, now: UtcTimestamp) -> Finding {
    match inputs.disk_encryption {
        Some(DiskEncryption::On) => Finding::ok("The data folder's disk is encrypted."),
        Some(DiskEncryption::Off) => Finding::new(
            HealthState::Alert,
            "off",
            "The data folder's disk is not encrypted.",
        ),
        None => unknown_or_attested(
            inputs,
            AttestationCode::DiskEncryption,
            now,
            "Disk encryption could not be read.",
        ),
    }
}

/// The UTC calendar day of `now`.
fn utc_date(now: UtcTimestamp) -> Option<Date> {
    jiff::Timestamp::from_second(now.unix_seconds())
        .ok()
        .map(|timestamp| timestamp.to_zoned(TimeZone::UTC).date())
}

/// Warning within the lab's days of the end date, alert after it; Home editions and
/// outdated macOS are alerts; Windows 10 needs attested Extended Security Updates.
pub(super) fn os_support(
    inputs: &HealthInputs,
    settings: &OperationsSettings,
    now: UtcTimestamp,
) -> Finding {
    let unknown = |summary| unknown_or_attested(inputs, AttestationCode::OsSupport, now, summary);
    let (Some(os), Some(today)) = (&inputs.os, utc_date(now)) else {
        return unknown("The operating system could not be read.");
    };
    let support = host::assess(os);
    let product = support.product;
    let finding = match support.status {
        SupportStatus::Until(end) => until(&product, end, today, settings),
        SupportStatus::HomeEdition => Finding::new(
            HealthState::Alert,
            "home_edition",
            format!("{product}: Home editions are not supported for laboratory use."),
        ),
        SupportStatus::ExtendedSecurityUpdates { last } if today > last => ended(&product, last),
        SupportStatus::ExtendedSecurityUpdates { last } => if valid_attestation(
            inputs,
            AttestationCode::OsSupport,
            now,
        )
        .is_some()
        {
            Finding::new(
                HealthState::Attested,
                "extended_security_updates",
                format!("{product}: Extended Security Updates attested."),
            )
        } else {
            Finding::new(
                HealthState::Alert,
                "needs_extended_security_updates",
                format!(
                    "{product}: supported only under Extended Security Updates; attest enrolment."
                ),
            )
        }
        .threshold(format!("ends {last}")),
        SupportStatus::MacosRecent => Finding::ok(format!(
            "{product}: a current or one of the two previous major versions (heuristic)."
        )),
        SupportStatus::MacosOutdated => Finding::new(
            HealthState::Alert,
            "outdated",
            format!("{product}: older than the two previous major versions."),
        ),
        SupportStatus::Unknown => {
            unknown("This operating system version is not in the support table.")
        }
    };
    finding.value(product)
}

fn until(product: &str, end: Date, today: Date, settings: &OperationsSettings) -> Finding {
    if today > end {
        return ended(product, end);
    }
    let days_left = today.until(end).map_or(0, |span| span.get_days());
    let state = if i64::from(days_left) <= i64::from(settings.os_support_warn_days) {
        HealthState::Warning
    } else {
        HealthState::Ok
    };
    Finding::new(
        state,
        "ending",
        format!("{product}: security updates until {end} ({days_left} days)."),
    )
    .threshold(format!("ends {end}"))
}

fn ended(product: &str, end: Date) -> Finding {
    Finding::new(
        HealthState::Alert,
        "ended",
        format!("{product}: security updates ended on {end}."),
    )
    .threshold(format!("ended {end}"))
}

/// A web-engine change is a warning until a smoke check attested after it (NFR-33, L12).
pub(super) fn webview(inputs: &HealthInputs, now: UtcTimestamp) -> Finding {
    let Some(changed_at) = inputs.runs.webview_changed_at else {
        return Finding::ok("The web engine has not changed.");
    };
    let smoke_tested = valid_attestation(inputs, AttestationCode::WebviewSmoke, now)
        .is_some_and(|attestation| attestation.at >= changed_at);
    if smoke_tested {
        Finding::new(
            HealthState::Attested,
            "smoke_tested",
            "The changed web engine was smoke-tested.",
        )
    } else {
        Finding::new(
            HealthState::Warning,
            "changed",
            "The web engine changed; run the smoke check.",
        )
    }
}

/// Alert while a listener or watched folder is in error; a warning, by count, for files
/// in watched folders waiting too long or too large to read (result-files brief §6, F14).
pub(super) fn listeners(inputs: &HealthInputs) -> Finding {
    let listeners = &inputs.listeners;
    if listeners.enabled_in_error > 0 {
        let count = listeners.enabled_in_error;
        return Finding::new(
            HealthState::Alert,
            "listener_error",
            format!("{count} enabled listener(s) in error."),
        )
        .value(count.to_string());
    }
    let (waiting, too_large) = (listeners.files_waiting, listeners.files_too_large);
    if waiting > 0 {
        return Finding::new(
            HealthState::Warning,
            "files_waiting",
            format!("{waiting} result file(s) waiting more than 5 minutes to be read."),
        )
        .value(waiting.to_string());
    }
    if too_large > 0 {
        return Finding::new(
            HealthState::Warning,
            "file_too_large",
            format!("{too_large} result file(s) over the size limit were not read."),
        )
        .value(too_large.to_string());
    }
    Finding::ok("No enabled listener is in error.")
}

pub(super) fn keep_awake(inputs: &HealthInputs) -> Finding {
    let listeners = &inputs.listeners;
    if listeners.active == 0 {
        return Finding::ok("No listener is active; sleep prevention is not needed.");
    }
    match listeners.keep_awake {
        KeepAwakeState::Held => Finding::ok("Sleep is prevented while listeners are active."),
        KeepAwakeState::NotNeeded | KeepAwakeState::Unavailable => Finding::new(
            HealthState::Alert,
            "not_held",
            "Listeners are active but sleep is not prevented.",
        ),
    }
}

pub(super) fn unclean_shutdowns(inputs: &HealthInputs) -> Finding {
    match inputs.runs.unclean_stops_30d {
        0 => Finding::ok("No unclean stop in 30 days."),
        count => Finding::new(
            HealthState::Warning,
            "unclean_stops",
            format!("{count} unclean stop(s) in 30 days."),
        )
        .value(count.to_string()),
    }
}

/// Alert for 24 hours after a backward jump, and while the clock is behind the newest
/// record; time synchronization off is a warning, and unknown unless attested.
pub(super) fn clock(inputs: &HealthInputs, now: UtcTimestamp) -> Finding {
    let clock = &inputs.clock;
    let recent_jump = clock
        .last_backward_jump_at
        .is_some_and(|at| now.saturating_duration_since(at) < hours(CLOCK_ALERT_HOURS));
    if recent_jump {
        return Finding::new(
            HealthState::Alert,
            "backward_jump",
            "The clock moved backwards in the last 24 hours.",
        );
    }
    if clock.newest_record_at.is_some_and(|newest| newest > now) {
        return Finding::new(
            HealthState::Alert,
            "behind_records",
            "The clock is behind the newest audit record.",
        );
    }
    match clock.time_sync {
        Some(true) => Finding::ok("The clock is synchronized."),
        Some(false) => Finding::new(
            HealthState::Warning,
            "not_synchronized",
            "Time synchronization is off.",
        ),
        None => unknown_or_attested(
            inputs,
            AttestationCode::TimeSync,
            now,
            "Time synchronization could not be read.",
        ),
    }
}

/// The UPS cannot be read in M4: a warning until attested.
pub(super) fn ups(inputs: &HealthInputs, now: UtcTimestamp) -> Finding {
    if valid_attestation(inputs, AttestationCode::Ups, now).is_some() {
        Finding::new(
            HealthState::Attested,
            "attested",
            "A working UPS is attested.",
        )
    } else {
        Finding::new(
            HealthState::Warning,
            "not_attested",
            "The app cannot read the UPS; attest it.",
        )
    }
}
