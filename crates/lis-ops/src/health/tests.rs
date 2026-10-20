//! Boundary tests for every §6.1 rule. Expected states come from the brief's table, not
//! from the implementation: warning and alert levels are breached at their value.

use std::time::Duration;

use lis_core::UtcTimestamp;

use super::*;
use crate::host::{DiskEncryption, LinuxRelease, MacosVersion, OsIdentity, WindowsVersion};
use crate::policy::DEFAULT_SETTINGS;
use crate::runs::RunFacts;
use crate::updates::VersionFacts;

const MIN: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(3600);
const DAY: Duration = Duration::from_secs(86_400);
const GIB: u64 = 1 << 30;

fn ts(text: &str) -> UtcTimestamp {
    UtcTimestamp::parse(text).unwrap()
}

fn now() -> UtcTimestamp {
    ts("2026-10-13T12:00:00.000000000Z")
}

fn ago(duration: Duration) -> UtcTimestamp {
    let seconds = i64::try_from(duration.as_secs()).unwrap();
    let at = jiff::Timestamp::from_second(now().unix_seconds() - seconds).unwrap();
    ts(&format!("{at:.9}"))
}

fn attestation(code: AttestationCode, at: UtcTimestamp) -> Attestation {
    Attestation {
        code,
        by: "synthetic-user".into(),
        at,
        valid_until: at.checked_add(90 * DAY).unwrap(),
    }
}

/// Inputs under which every check is fine.
fn healthy() -> HealthInputs {
    HealthInputs {
        backup: BackupFacts {
            active_targets: 2,
            removable_targets: 1,
            backup_age: Some(Duration::ZERO),
            offhost_age: Some(HOUR),
            offline_age: Some(DAY),
            last_passed_restore_test_at: Some(ago(10 * DAY)),
            kit: KitFacts {
                last_export_at: Some(ago(20 * DAY)),
                outdated: false,
                last_confirmation_at: Some(ago(20 * DAY)),
            },
            artifact_problems: 0,
            failed_runs_in_a_row: 0,
            failed_copies_in_a_row: 0,
            snapshot_intact: Some(true),
        },
        anchors: AnchorFacts {
            offhost_targets: 1,
            lag: Some(5 * MIN),
        },
        audit_chain_findings: Some(0),
        disk: DiskFacts {
            available_bytes: Some(200 * GIB),
            database_bytes: GIB,
            growth_bytes_per_day: None,
        },
        disk_encryption: Some(DiskEncryption::On),
        os: Some(ubuntu("24.04")),
        runs: RunFacts {
            unacknowledged_gaps: 0,
            unclean_stops_30d: 0,
            webview_changed_at: None,
        },
        listeners: ListenerFacts {
            enabled_in_error: 0,
            files_waiting: 0,
            files_too_large: 0,
            active: 1,
            keep_awake: KeepAwakeState::Held,
        },
        clock: ClockFacts {
            last_backward_jump_at: None,
            newest_record_at: Some(ago(MIN)),
            time_sync: Some(true),
        },
        outbox: OutboxFacts {
            dead_letters: 0,
            oldest_pending_age: None,
        },
        version: VersionFacts {
            started_without_approval: false,
            awaiting_acceptance: false,
        },
        recovery_open: false,
        attestations: vec![attestation(AttestationCode::Ups, ago(DAY))],
    }
}

fn ubuntu(version: &str) -> OsIdentity {
    OsIdentity::Linux(LinuxRelease {
        id: "ubuntu".into(),
        version_id: version.into(),
        pretty_name: None,
    })
}

fn result_at(inputs: &HealthInputs, check: CheckCode, at: UtcTimestamp) -> CheckResult {
    evaluate(inputs, &DEFAULT_SETTINGS, at)
        .into_iter()
        .find(|result| result.check == check)
        .unwrap()
}

fn state(inputs: &HealthInputs, check: CheckCode) -> HealthState {
    result_at(inputs, check, now()).state
}

fn with(change: impl FnOnce(&mut HealthInputs)) -> HealthInputs {
    let mut inputs = healthy();
    change(&mut inputs);
    inputs
}

use HealthState::{Alert, Attested, Ok as Fine, Unknown, Warning};

#[test]
fn healthy_inputs_give_every_check_and_the_overall_state_ok() {
    let results = evaluate(&healthy(), &DEFAULT_SETTINGS, now());
    assert_eq!(results.len(), ALL_CHECKS.len());
    for result in &results {
        assert!(result.state.is_fine(), "{:?}", result);
        assert_eq!(result.next_action, None);
        assert_eq!(result.observed_at, now());
    }
    assert_eq!(overall(results.iter().map(|result| result.state)), Fine);
}

#[test]
fn backup_target_and_backup_age_levels() {
    let check = CheckCode::BackupConfigured;
    assert_eq!(state(&with(|i| i.backup.active_targets = 0), check), Alert);
    let age = |age| state(&with(|i| i.backup.backup_age = age), CheckCode::BackupAge);
    assert_eq!(age(None), Alert);
    // Two missed hourly intervals since 2026-10-13 (L2 note in the brief's §11).
    assert_eq!(age(Some(2 * HOUR - Duration::from_secs(1))), Fine);
    assert_eq!(age(Some(2 * HOUR)), Warning);
    assert_eq!(age(Some(4 * HOUR - Duration::from_secs(1))), Warning);
    assert_eq!(age(Some(4 * HOUR)), Alert);
}

#[test]
fn backup_failures_warn_from_one_and_alert_at_three_in_a_row() {
    let failures = |runs, copies| {
        let inputs = with(|i| {
            i.backup.failed_runs_in_a_row = runs;
            i.backup.failed_copies_in_a_row = copies;
        });
        result_at(&inputs, CheckCode::BackupFailures, now())
    };
    assert_eq!(failures(0, 0).state, Fine);
    assert_eq!(failures(1, 0).state, Warning);
    assert_eq!(failures(0, 1).state, Warning);
    assert_eq!(failures(2, 2).state, Warning);
    assert_eq!(failures(3, 0).state, Alert);
    let one_target = failures(1, 3);
    assert_eq!(one_target.state, Alert);
    assert_eq!(one_target.value.as_deref(), Some("3"));
    assert!(one_target.summary.contains("failed"), "{one_target:?}");
    assert_eq!(one_target.owner_capability, "backup_restore");
}

#[test]
fn a_damaged_snapshot_alerts_until_an_intact_one_and_is_unknown_before_any() {
    let integrity = |intact| {
        let inputs = with(|i| i.backup.snapshot_intact = intact);
        result_at(&inputs, CheckCode::DatabaseIntegrity, now())
    };
    assert_eq!(integrity(None).state, Unknown);
    assert_eq!(integrity(Some(true)).state, Fine);
    let damaged = integrity(Some(false));
    assert_eq!(damaged.state, Alert);
    assert_eq!(damaged.detail_code.as_deref(), Some("snapshot_not_intact"));
    assert!(damaged.summary.contains("damaged"), "{damaged:?}");
    assert_eq!(damaged.owner_capability, "manage_configuration");
    assert!(damaged.next_action.is_some());
}

#[test]
fn offhost_and_offline_copy_ages() {
    let offhost = |age| {
        state(
            &with(|i| i.backup.offhost_age = age),
            CheckCode::OffhostCopyAge,
        )
    };
    assert_eq!(offhost(None), Warning);
    assert_eq!(offhost(Some(24 * HOUR - MIN)), Fine);
    assert_eq!(offhost(Some(24 * HOUR)), Warning);
    let offline = |age| {
        state(
            &with(|i| i.backup.offline_age = age),
            CheckCode::OfflineCopyAge,
        )
    };
    assert_eq!(offline(None), Warning);
    assert_eq!(offline(Some(7 * DAY - MIN)), Fine);
    assert_eq!(offline(Some(7 * DAY)), Warning);
    let no_removable = with(|i| i.backup.removable_targets = 0);
    assert_eq!(state(&no_removable, CheckCode::OfflineCopyAge), Warning);
}

#[test]
fn restore_test_and_recovery_kit() {
    let test = |at| {
        state(
            &with(|i| i.backup.last_passed_restore_test_at = at),
            CheckCode::RestoreTestAge,
        )
    };
    assert_eq!(test(None), Alert);
    assert_eq!(test(Some(ago(90 * DAY - MIN))), Fine);
    assert_eq!(test(Some(ago(90 * DAY))), Warning);
    let kit = |change: fn(&mut KitFacts)| {
        state(&with(|i| change(&mut i.backup.kit)), CheckCode::RecoveryKit)
    };
    assert_eq!(kit(|k| k.last_export_at = None), Alert);
    assert_eq!(kit(|k| k.outdated = true), Warning);
    assert_eq!(kit(|k| k.last_confirmation_at = None), Warning);
    assert_eq!(
        kit(|k| k.last_confirmation_at = Some(ago(90 * DAY))),
        Warning
    );
    assert_eq!(kit(|k| k.last_confirmation_at = Some(ago(89 * DAY))), Fine);
}

#[test]
fn anchor_lag_levels_and_missing_offhost_target() {
    let lag = |lag| state(&with(|i| i.anchors.lag = lag), CheckCode::AnchorLag);
    assert_eq!(lag(None), Fine);
    assert_eq!(lag(Some(30 * MIN - Duration::from_secs(1))), Fine);
    assert_eq!(lag(Some(30 * MIN)), Warning);
    assert_eq!(lag(Some(24 * HOUR)), Alert);
    let none = with(|i| i.anchors.offhost_targets = 0);
    assert_eq!(state(&none, CheckCode::AnchorLag), Alert);
}

#[test]
fn audit_chain_artifacts_gaps_unclean_stops_and_recovery() {
    let chain = |findings| {
        state(
            &with(|i| i.audit_chain_findings = findings),
            CheckCode::AuditChain,
        )
    };
    assert_eq!(chain(None), Unknown);
    assert_eq!(chain(Some(2)), Alert);
    let artifacts = with(|i| i.backup.artifact_problems = 1);
    assert_eq!(state(&artifacts, CheckCode::ArtifactStore), Alert);
    let gaps = with(|i| i.runs.unacknowledged_gaps = 1);
    assert_eq!(state(&gaps, CheckCode::ListeningGaps), Warning);
    let unclean = with(|i| i.runs.unclean_stops_30d = 1);
    assert_eq!(state(&unclean, CheckCode::UncleanShutdowns), Warning);
    let recovery = with(|i| i.recovery_open = true);
    assert_eq!(state(&recovery, CheckCode::RecoveryReconciliation), Alert);
    assert_eq!(state(&healthy(), CheckCode::CloudServices), Fine);
}

#[test]
fn disk_free_uses_the_larger_warning_level_and_the_alert_level() {
    let free = |available, database| {
        let inputs = with(|i| {
            i.disk.available_bytes = available;
            i.disk.database_bytes = database;
        });
        state(&inputs, CheckCode::DiskFree)
    };
    assert_eq!(free(None, GIB), Unknown);
    assert_eq!(free(Some(10 * GIB), GIB), Fine);
    assert_eq!(free(Some(10 * GIB - 1), GIB), Warning);
    assert_eq!(free(Some(15 * GIB - 1), 5 * GIB), Warning);
    assert_eq!(free(Some(15 * GIB), 5 * GIB), Fine);
    assert_eq!(free(Some(6 * GIB), 5 * GIB), Warning);
    assert_eq!(free(Some(6 * GIB - 1), 5 * GIB), Alert);
}

#[test]
fn disk_encryption_is_attested_only_while_unknown() {
    let encryption = |value, attestations: Vec<Attestation>| {
        let inputs = with(|i| {
            i.disk_encryption = value;
            i.attestations = attestations;
        });
        state(&inputs, CheckCode::DiskEncryption)
    };
    let valid = attestation(AttestationCode::DiskEncryption, ago(DAY));
    let expired = attestation(AttestationCode::DiskEncryption, ago(90 * DAY));
    let other = attestation(AttestationCode::Ups, ago(DAY));
    assert_eq!(
        encryption(Some(DiskEncryption::Off), vec![valid.clone()]),
        Alert
    );
    assert_eq!(encryption(None, vec![]), Unknown);
    assert_eq!(encryption(None, vec![valid]), Attested);
    assert_eq!(encryption(None, vec![expired]), Unknown);
    assert_eq!(encryption(None, vec![other]), Unknown);
}

#[test]
fn os_support_warns_within_the_lab_days_and_alerts_after_the_end() {
    let at = |text: &str| result_at(&healthy(), CheckCode::OsSupport, ts(text)).state;
    // Ubuntu 24.04: standard security support until 2029-05-31.
    assert_eq!(at("2029-03-01T23:59:59.000000000Z"), Fine);
    assert_eq!(at("2029-03-02T00:00:00.000000000Z"), Warning);
    assert_eq!(at("2029-05-31T23:59:59.000000000Z"), Warning);
    assert_eq!(at("2029-06-01T00:00:00.000000000Z"), Alert);
}

fn windows(build: u32, edition: &str) -> OsIdentity {
    OsIdentity::Windows(WindowsVersion {
        build,
        display_version: None,
        edition_id: edition.into(),
    })
}

#[test]
fn os_support_alerts_for_home_and_windows_10_without_attested_updates() {
    let os = |os, attestations: Vec<Attestation>, at: &str| {
        let inputs = with(|i| {
            i.os = Some(os);
            i.attestations = attestations;
        });
        result_at(&inputs, CheckCode::OsSupport, ts(at)).state
    };
    let esu = |at| vec![attestation(AttestationCode::OsSupport, at)];
    let today = "2026-10-13T12:00:00.000000000Z";
    assert_eq!(os(windows(26200, "Core"), esu(ago(DAY)), today), Alert);
    assert_eq!(os(windows(19045, "Professional"), vec![], today), Alert);
    assert_eq!(
        os(windows(19045, "Professional"), esu(ago(DAY)), today),
        Attested
    );
    let after_esu = "2028-10-11T00:00:00.000000000Z";
    let attested_then = esu(ts("2028-10-01T00:00:00.000000000Z"));
    assert_eq!(
        os(windows(19045, "Professional"), attested_then, after_esu),
        Alert
    );
    assert_eq!(os(windows(26200, "Professional"), vec![], today), Fine);
    assert_eq!(os(windows(26100, "Professional"), vec![], today), Warning);
}

#[test]
fn os_support_unknown_versions_and_macos_heuristic() {
    let os = |os: Option<OsIdentity>, attestations: Vec<Attestation>| {
        let inputs = with(|i| {
            i.os = os;
            i.attestations = attestations;
        });
        state(&inputs, CheckCode::OsSupport)
    };
    let mac = |major| {
        Some(OsIdentity::Macos(MacosVersion {
            product_version: format!("{major}.0"),
            major,
        }))
    };
    assert_eq!(os(None, vec![]), Unknown);
    assert_eq!(os(Some(ubuntu("25.10")), vec![]), Unknown);
    let attested = vec![attestation(AttestationCode::OsSupport, ago(DAY))];
    assert_eq!(os(Some(ubuntu("25.10")), attested), Attested);
    assert_eq!(os(mac(15), vec![]), Fine);
    assert_eq!(os(mac(14), vec![]), Alert);
}

#[test]
fn a_web_engine_change_warns_until_a_later_smoke_check_is_attested() {
    let webview = |attested_at: Option<UtcTimestamp>| {
        let inputs = with(|i| {
            i.runs.webview_changed_at = Some(ago(2 * DAY));
            i.attestations = attested_at
                .map(|at| attestation(AttestationCode::WebviewSmoke, at))
                .into_iter()
                .collect();
        });
        state(&inputs, CheckCode::Webview)
    };
    assert_eq!(webview(None), Warning);
    assert_eq!(webview(Some(ago(3 * DAY))), Warning);
    assert_eq!(webview(Some(ago(DAY))), Attested);
}

#[test]
fn listeners_and_keep_awake() {
    let errors = with(|i| i.listeners.enabled_in_error = 1);
    assert_eq!(state(&errors, CheckCode::Listeners), Alert);
    // Files waiting or too large in a watched folder are a counted warning, not an error.
    let waiting = with(|i| i.listeners.files_waiting = 2);
    assert_eq!(state(&waiting, CheckCode::Listeners), Warning);
    let too_large = with(|i| i.listeners.files_too_large = 1);
    assert_eq!(state(&too_large, CheckCode::Listeners), Warning);
    let both = with(|i| {
        i.listeners.enabled_in_error = 1;
        i.listeners.files_waiting = 2;
    });
    assert_eq!(state(&both, CheckCode::Listeners), Alert);
    let awake = |active, keep_awake| {
        let inputs = with(|i| {
            i.listeners.active = active;
            i.listeners.keep_awake = keep_awake;
        });
        state(&inputs, CheckCode::KeepAwake)
    };
    assert_eq!(awake(2, KeepAwakeState::Unavailable), Alert);
    assert_eq!(awake(2, KeepAwakeState::NotNeeded), Alert);
    assert_eq!(awake(0, KeepAwakeState::Unavailable), Fine);
}

#[test]
fn clock_jumps_records_and_time_sync() {
    let clock =
        |change: fn(&mut ClockFacts)| state(&with(|i| change(&mut i.clock)), CheckCode::Clock);
    assert_eq!(
        clock(|c| c.last_backward_jump_at = Some(ago(24 * HOUR - MIN))),
        Alert
    );
    assert_eq!(
        clock(|c| c.last_backward_jump_at = Some(ago(24 * HOUR))),
        Fine
    );
    assert_eq!(
        clock(|c| c.newest_record_at = Some(now().checked_add(MIN).unwrap())),
        Alert
    );
    assert_eq!(clock(|c| c.time_sync = Some(false)), Warning);
    assert_eq!(clock(|c| c.time_sync = None), Unknown);
    let attested = with(|i| {
        i.clock.time_sync = None;
        i.attestations = vec![attestation(AttestationCode::TimeSync, ago(DAY))];
    });
    assert_eq!(state(&attested, CheckCode::Clock), Attested);
}

#[test]
fn outbox_version_and_ups() {
    let outbox = |dead, pending| {
        let inputs = with(|i| {
            i.outbox.dead_letters = dead;
            i.outbox.oldest_pending_age = pending;
        });
        state(&inputs, CheckCode::Outbox)
    };
    assert_eq!(outbox(1, None), Warning);
    assert_eq!(outbox(0, Some(HOUR - MIN)), Fine);
    assert_eq!(outbox(0, Some(HOUR)), Warning);
    let unapproved = with(|i| {
        i.version.started_without_approval = true;
        i.version.awaiting_acceptance = true;
    });
    assert_eq!(state(&unapproved, CheckCode::Version), Alert);
    let awaiting = with(|i| i.version.awaiting_acceptance = true);
    assert_eq!(state(&awaiting, CheckCode::Version), Warning);
    let no_ups = with(|i| i.attestations.clear());
    assert_eq!(state(&no_ups, CheckCode::Ups), Warning);
}

#[test]
fn results_carry_owner_next_action_and_attestation() {
    let inputs = with(|i| {
        i.backup.active_targets = 0;
        i.disk_encryption = None;
        i.attestations = vec![attestation(AttestationCode::DiskEncryption, ago(DAY))];
    });
    let target = result_at(&inputs, CheckCode::BackupConfigured, now());
    assert_eq!(target.owner_capability, "backup_restore");
    assert_eq!(target.detail_code.as_deref(), Some("no_target"));
    assert!(target.next_action.is_some());
    assert!(!target.attestable);
    let encryption = result_at(&inputs, CheckCode::DiskEncryption, now());
    assert!(encryption.attestable);
    assert_eq!(encryption.attestation.unwrap().at, ago(DAY));
    let json = serde_json::to_value(result_at(&inputs, CheckCode::Version, now())).unwrap();
    assert_eq!(json["ownerCapability"], "approve_configuration");
    assert_eq!(json["state"], "ok");
    let results = evaluate(&inputs, &DEFAULT_SETTINGS, now());
    assert_eq!(overall(results.iter().map(|result| result.state)), Alert);
}
