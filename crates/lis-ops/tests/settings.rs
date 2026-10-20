//! Versioned and append-only runtime records on a real SQLite file: operations settings,
//! health attestations and history, and media disposals (REQ-OPS-003, REQ-OPS-008).
//! Synthetic data only.

#![cfg(test)]

use std::time::Duration;

use lis_core::{Capability, CorrelationId, Principal, UtcTimestamp};
use lis_ops::RuntimeError;
use lis_ops::disposals::{DisposalMethod, RecordMediaDisposalInput};
use lis_ops::health::{AttestCheckInput, AttestationCode, CheckCode, CheckResult, HealthState};
use lis_ops::policy::{DEFAULT_SETTINGS, OperationsSettings};
use lis_ops::settings::{OperationsSettingsView, SetOperationsSettingsInput};
use lis_store::testing::{TestDatabase, principal, test_start};
use lis_store::{CommandContext, DATABASE_FILE, StoreError, lint_schema};
use rusqlite::Connection;

fn open() -> TestDatabase {
    TestDatabase::open(&[lis_ops::SCHEMA, lis_ops::RUNTIME_SCHEMA]).unwrap()
}

fn write<T, F>(test: &TestDatabase, principal: &Principal, command: F) -> Result<T, RuntimeError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, RuntimeError> + Send + 'static,
{
    test.db
        .write_blocking(principal.clone(), CorrelationId::generate(), command)
}

fn at(seconds: u64) -> UtcTimestamp {
    test_start()
        .checked_add(Duration::from_secs(seconds))
        .unwrap()
}

fn connection(test: &TestDatabase) -> Connection {
    Connection::open(test.data_dir().join(DATABASE_FILE)).unwrap()
}

fn audit(test: &TestDatabase, action: &str) -> Vec<(String, Option<String>, String)> {
    connection(test)
        .prepare("SELECT stream, reason, details FROM audit_events WHERE action = ?1 ORDER BY seq")
        .unwrap()
        .query_map([action], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn is_denied(error: &RuntimeError) -> bool {
    matches!(error, RuntimeError::Store(StoreError::Denied(_)))
}

fn configurator() -> Principal {
    principal(&[Capability::ManageConfiguration])
}

#[test]
fn the_runtime_schema_passes_the_lint() {
    assert_eq!(
        lint_schema(&[lis_ops::SCHEMA, lis_ops::RUNTIME_SCHEMA]).unwrap(),
        []
    );
}

fn set(
    test: &TestDatabase,
    principal: &Principal,
    expected_version: Option<i64>,
    values: OperationsSettings,
    reason: Option<&str>,
) -> Result<OperationsSettingsView, RuntimeError> {
    let input = SetOperationsSettingsInput {
        expected_version,
        values,
        reason: reason.map(str::to_owned),
    };
    write(test, principal, move |ctx| {
        lis_ops::settings::set_operations_settings(ctx, input)
    })
}

#[test]
fn settings_default_until_set_then_version_with_a_reason() {
    let test = open();
    let view = test
        .db
        .read_blocking(lis_ops::settings::get_operations_settings)
        .unwrap();
    assert_eq!((view.version, view.values), (None, DEFAULT_SETTINGS));
    assert_eq!(view.set_by, None);

    let half_hourly = OperationsSettings {
        backup_interval_minutes: 30,
        backup_warn_minutes: 60,
        ..DEFAULT_SETTINGS
    };
    let first = set(&test, &configurator(), None, half_hourly, None).unwrap();
    assert_eq!((first.version, first.values), (Some(1), half_hourly));
    assert_eq!(first.defaults, DEFAULT_SETTINGS);
    assert_eq!(first.set_at, Some(test_start()));
    let unexplained = set(&test, &configurator(), Some(1), DEFAULT_SETTINGS, None);
    assert_eq!(unexplained.unwrap_err().field_errors()[0].field, "reason");
    let second = set(
        &test,
        &configurator(),
        Some(1),
        DEFAULT_SETTINGS,
        Some("Back to the defaults per QP-7"),
    )
    .unwrap();
    assert_eq!(second.version, Some(2));
    let current = test.db.read_blocking(lis_ops::settings::current).unwrap();
    assert_eq!(current, DEFAULT_SETTINGS);

    let events = audit(&test, lis_ops::settings::SET_SETTINGS);
    assert_eq!(events.len(), 2);
    assert!(
        events[0]
            .2
            .contains("\"changed\":[\"backup_interval_minutes\",\"backup_warn_minutes\"]")
    );
    assert_eq!(
        events[1].1.as_deref(),
        Some("Back to the defaults per QP-7")
    );
}

#[test]
fn settings_refuse_stale_versions_bad_values_and_missing_capability() {
    let test = open();
    let denied = set(
        &test,
        &principal(&[Capability::ManageUsers]),
        None,
        DEFAULT_SETTINGS,
        None,
    );
    assert!(is_denied(&denied.unwrap_err()));
    let stale = set(&test, &configurator(), Some(3), DEFAULT_SETTINGS, None);
    assert!(matches!(stale, Err(RuntimeError::SettingsVersionConflict)));
    let invalid = OperationsSettings {
        backup_interval_minutes: 4,
        keep_recent: 97,
        backup_alert_minutes: 30,
        ..DEFAULT_SETTINGS
    };
    let error = set(&test, &configurator(), None, invalid, None).unwrap_err();
    let fields: Vec<&str> = error
        .field_errors()
        .iter()
        .map(|e| e.field.as_str())
        .collect();
    assert_eq!(
        fields,
        ["backupIntervalMinutes", "keepRecent", "backupAlertMinutes"]
    );
    set(&test, &configurator(), None, DEFAULT_SETTINGS, None).unwrap();
    let stale = set(
        &test,
        &configurator(),
        None,
        DEFAULT_SETTINGS,
        Some("again"),
    );
    assert!(matches!(stale, Err(RuntimeError::SettingsVersionConflict)));
}

#[test]
fn settings_versions_are_append_only_and_checked_by_the_store() {
    let test = open();
    set(&test, &configurator(), None, DEFAULT_SETTINGS, None).unwrap();
    let conn = connection(&test);
    let insert = |version: i64, interval: u32, reason: Option<&str>| {
        conn.execute(
            "INSERT INTO ops_settings_versions SELECT ?1, ?2, keep_recent, keep_daily, \
             keep_weekly, backup_warn_minutes, backup_alert_minutes, offhost_warn_hours, \
             offline_warn_days, restore_test_warn_days, kit_confirm_warn_days, \
             anchor_warn_minutes, anchor_incident_hours, disk_free_warn_gib, \
             os_support_warn_days, clock_jump_seconds, ?3, set_by, set_at \
             FROM ops_settings_versions WHERE version = 1",
            rusqlite::params![version, interval, reason],
        )
    };
    assert!(insert(3, 15, Some("gap")).is_err());
    assert!(insert(2, 15, None).is_err());
    assert!(insert(2, 4, Some("out of range")).is_err());
    assert!(insert(2, 30, Some("ok")).is_ok());
    for sql in [
        "UPDATE ops_settings_versions SET keep_recent = 9",
        "DELETE FROM ops_settings_versions",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

fn attest(
    test: &TestDatabase,
    principal: &Principal,
    check: CheckCode,
    note: &str,
) -> Result<lis_ops::health::Attestation, RuntimeError> {
    let input = AttestCheckInput {
        check,
        note: note.into(),
    };
    write(test, principal, move |ctx| {
        lis_ops::health::attest_check(ctx, input)
    })
}

fn valid_at(test: &TestDatabase, seconds: u64) -> Vec<(AttestationCode, UtcTimestamp)> {
    test.db
        .read_blocking(move |conn| lis_ops::health::valid_attestations(conn, at(seconds)))
        .unwrap()
        .into_iter()
        .map(|attestation| (attestation.code, attestation.at))
        .collect()
}

#[test]
fn attestations_cover_attestable_checks_for_ninety_days() {
    let test = open();
    let denied = attest(
        &test,
        &principal(&[Capability::BackupRestore]),
        CheckCode::Ups,
        "UPS",
    );
    assert!(is_denied(&denied.unwrap_err()));
    let not_attestable = attest(&test, &configurator(), CheckCode::BackupAge, "no");
    assert_eq!(
        not_attestable.unwrap_err().field_errors()[0].code,
        "not_attestable"
    );
    let blank = attest(&test, &configurator(), CheckCode::Ups, " ");
    assert_eq!(blank.unwrap_err().field_errors()[0].field, "note");

    let first = attest(
        &test,
        &configurator(),
        CheckCode::Ups,
        "UPS tested 2026-01-02",
    )
    .unwrap();
    assert_eq!(first.valid_until, at(90 * 86_400));
    test.clock.set(at(60));
    attest(&test, &configurator(), CheckCode::Ups, "UPS retested").unwrap();
    attest(
        &test,
        &configurator(),
        CheckCode::Clock,
        "NTP checked on the router",
    )
    .unwrap();
    assert_eq!(
        valid_at(&test, 120),
        [
            (AttestationCode::TimeSync, at(60)),
            (AttestationCode::Ups, at(60))
        ]
    );
    assert_eq!(valid_at(&test, 60 + 90 * 86_400), []);
    let event = &audit(&test, lis_ops::health::ATTEST_CHECK)[0];
    assert_eq!(event.1.as_deref(), Some("UPS tested 2026-01-02"));
    assert!(event.2.contains("\"attestation\":\"ups\""), "{}", event.2);

    let conn = connection(&test);
    let too_long = conn.execute(
        "INSERT INTO ops_attestations VALUES ('x', 'ups', 'u', ?1, ?2)",
        rusqlite::params![at(0), at(91 * 86_400)],
    );
    assert!(too_long.is_err());
    assert!(conn.execute("DELETE FROM ops_attestations", []).is_err());
}

fn result(check: CheckCode, state: HealthState, seconds: u64) -> CheckResult {
    CheckResult {
        check,
        label: "Synthetic".into(),
        state,
        detail_code: None,
        summary: "Synthetic summary.".into(),
        value: Some("35 min".into()),
        threshold: None,
        observed_at: at(seconds),
        owner_capability: "backup_restore".into(),
        next_action: None,
        attestable: false,
        attestation: None,
    }
}

fn record(test: &TestDatabase, results: Vec<CheckResult>) -> usize {
    write(test, &Principal::system("engine", []), move |ctx| {
        lis_ops::health::record_state_changes(ctx, &results)
    })
    .unwrap()
}

#[test]
fn health_history_keeps_only_state_changes() {
    let test = open();
    let both = |state, seconds| {
        vec![
            result(CheckCode::BackupAge, state, seconds),
            result(CheckCode::Ups, HealthState::Warning, seconds),
        ]
    };
    assert_eq!(record(&test, both(HealthState::Ok, 0)), 2);
    assert_eq!(record(&test, both(HealthState::Ok, 300)), 0);
    assert_eq!(record(&test, both(HealthState::Warning, 600)), 1);
    assert_eq!(record(&test, both(HealthState::Ok, 900)), 1);
    let history = test
        .db
        .read_blocking(|conn| {
            lis_ops::health::list_health_history(conn, Some(CheckCode::BackupAge), 10)
        })
        .unwrap();
    let states: Vec<(HealthState, UtcTimestamp)> = history
        .iter()
        .map(|event| (event.state, event.observed_at))
        .collect();
    assert_eq!(
        states,
        [
            (HealthState::Ok, at(900)),
            (HealthState::Warning, at(600)),
            (HealthState::Ok, at(0))
        ]
    );
    let all = test
        .db
        .read_blocking(|conn| lis_ops::health::list_health_history(conn, None, 2))
        .unwrap();
    assert_eq!(all.len(), 3, "the extra row proves more history exists");
    assert!(
        connection(&test)
            .execute("UPDATE ops_health_events SET state = 'ok'", [])
            .is_err()
    );
}

fn dispose(
    test: &TestDatabase,
    principal: &Principal,
    medium: &str,
    identifier: Option<&str>,
) -> Result<lis_ops::disposals::MediaDisposalView, RuntimeError> {
    let input = RecordMediaDisposalInput {
        medium: medium.into(),
        method: DisposalMethod::Destroyed,
        identifier: identifier.map(str::to_owned),
        note: "Shredded by the vendor; certificate on file".into(),
    };
    write(test, principal, move |ctx| {
        lis_ops::disposals::record_media_disposal(ctx, input)
    })
}

#[test]
fn media_disposals_are_recorded_by_backup_operators_and_never_changed() {
    let test = open();
    let operator = principal(&[Capability::BackupRestore]);
    let denied = dispose(&test, &configurator(), "Backup drive B", None);
    assert!(is_denied(&denied.unwrap_err()));
    let invalid = dispose(&test, &operator, " ", Some(&"9".repeat(101))).unwrap_err();
    let fields: Vec<&str> = invalid
        .field_errors()
        .iter()
        .map(|e| e.field.as_str())
        .collect();
    assert_eq!(fields, ["medium", "identifier"]);

    let first = dispose(&test, &operator, "Backup drive B (USB)", Some("SN-0001")).unwrap();
    test.clock.set(at(60));
    dispose(&test, &operator, "Old host disk", None).unwrap();
    let list = test
        .db
        .read_blocking(lis_ops::disposals::list_media_disposals)
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1], first);
    assert_eq!(list[0].identifier, None);
    let json = serde_json::to_value(&first).unwrap();
    assert_eq!(json["method"], "destroyed");
    assert!(json.get("disposalId").is_some() && json.get("recordedBy").is_some());

    let event = &audit(&test, lis_ops::disposals::RECORD_MEDIA_DISPOSAL)[0];
    assert_eq!(event.0, "security");
    assert!(!event.2.contains("Backup drive") && !event.2.contains("SN-0001"));
    let conn = connection(&test);
    assert!(
        conn.execute("UPDATE ops_media_disposals SET medium = 'x'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM ops_media_disposals", []).is_err());
}

#[test]
fn health_current_follows_committed_observations_across_clock_rollback_and_equal_times() {
    let test = open();
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Ok, 600)]
        ),
        1
    );
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Warning, 300)]
        ),
        1
    );
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Warning, 301)]
        ),
        0
    );
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Ok, 301)]
        ),
        1
    );
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Ok, 301)]
        ),
        0
    );
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Alert, 200)]
        ),
        1
    );
    let conn = connection(&test);
    let events: i64 = conn
        .query_row("SELECT count(*) FROM ops_health_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 4, "only actual state changes append observations");
    let state: String = conn.query_row("SELECT e.state FROM ops_health_current c JOIN ops_health_events e ON e.id=c.event_id WHERE c.check_code='backup_age'",[],|r|r.get(0)).unwrap();
    assert_eq!(state, "alert");
    let current = lis_ops::health::current_health_event(&conn, CheckCode::BackupAge)
        .unwrap()
        .unwrap();
    assert_eq!(current.state, HealthState::Alert);
    assert_eq!(current.observed_at, at(200));
    assert_eq!(
        lis_ops::health::list_health_history(&conn, Some(CheckCode::BackupAge), 1).unwrap()[0]
            .observed_at,
        at(600),
        "history display order is distinct from current causal state"
    );
}

#[test]
fn interrupted_health_observation_rolls_back_both_event_and_current_pointer() {
    let test = open();
    record(
        &test,
        vec![result(CheckCode::BackupAge, HealthState::Ok, 301)],
    );
    let failed: Result<(), RuntimeError> = write(&test, &Principal::system("engine", []), |ctx| {
        assert_eq!(
            lis_ops::health::record_state_changes(
                ctx,
                &[result(CheckCode::BackupAge, HealthState::Alert, 200)]
            )?,
            1
        );
        Err(RuntimeError::NotFound("synthetic rollback"))
    });
    assert!(failed.is_err());
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Ok, 301)]
        ),
        0
    );
    let conn = connection(&test);
    let history =
        lis_ops::health::list_health_history(&conn, Some(CheckCode::BackupAge), 10).unwrap();
    assert_eq!(history.len(), 1);
    let current = lis_ops::health::current_health_event(&conn, CheckCode::BackupAge)
        .unwrap()
        .unwrap();
    assert_eq!(current, history[0]);
    assert_eq!(current.state, HealthState::Ok);
}

#[test]
fn legacy_health_first_evaluation_records_real_baseline_without_guessing_history() {
    let test = open();
    let conn = connection(&test);
    let legacy = lis_ops::health::HealthEventId::generate();
    conn.execute("INSERT INTO ops_health_events(id,check_code,state,observed_at) VALUES(?1,'backup_age','ok',?2)",rusqlite::params![legacy,at(600)]).unwrap();
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Ok, 300)]
        ),
        1
    );
    assert_eq!(
        record(
            &test,
            vec![result(CheckCode::BackupAge, HealthState::Ok, 301)]
        ),
        0
    );
    let preserved: (String, UtcTimestamp) = conn
        .query_row(
            "SELECT state,observed_at FROM ops_health_events WHERE id=?1",
            [legacy],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(preserved, ("ok".into(), at(600)));
    let pointer: lis_ops::health::HealthEventId = conn
        .query_row(
            "SELECT event_id FROM ops_health_current WHERE check_code='backup_age'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(pointer, legacy);
}

#[test]
fn health_current_migration_preserves_legacy_events_and_does_not_backfill() {
    let conn = Connection::open_in_memory().unwrap();
    for migration in lis_ops::SCHEMA.migrations {
        conn.execute_batch(migration.sql).unwrap();
    }
    for migration in &lis_ops::RUNTIME_MIGRATIONS[..3] {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute("INSERT INTO ops_health_events(id,check_code,state,observed_at) VALUES('legacy','backup_age','ok',?1)",[at(600)]).unwrap();
    conn.execute_batch(lis_ops::RUNTIME_MIGRATIONS[3].sql)
        .unwrap();
    let original: (String, String, String, UtcTimestamp) = conn
        .query_row(
            "SELECT id,check_code,state,observed_at FROM ops_health_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        original,
        ("legacy".into(), "backup_age".into(), "ok".into(), at(600))
    );
    let count: i64 = conn
        .query_row("SELECT count(*) FROM ops_health_current", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    assert!(
        lis_ops::health::current_health_event(&conn, CheckCode::BackupAge)
            .unwrap()
            .is_none()
    );
    assert!(
        conn.execute(
            "INSERT INTO ops_health_current VALUES('disk_free','legacy')",
            []
        )
        .is_err()
    );
}
