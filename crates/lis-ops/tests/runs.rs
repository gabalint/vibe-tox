//! App runs, listening gaps, update approvals and version acceptance on a real SQLite
//! file (REQ-OPS-002, REQ-OPS-006, REQ-OPS-013, NFR-24). Synthetic data only.

#![cfg(test)]

use std::time::Duration;

use lis_core::{Capability, CorrelationId, Principal, UtcTimestamp};
use lis_ops::RuntimeError;
use lis_ops::backup::RunId;
use lis_ops::health::ClockStep;
use lis_ops::runs::{AcknowledgeGapInput, AppRunId, RunStart, StartedRun, StopKind};
use lis_ops::updates::{
    AcceptVersionInput, NewUpdateApproval, VersionStatus, WithdrawUpdateApprovalInput,
};
use lis_store::testing::{TestDatabase, principal, test_start};
use lis_store::{CommandContext, DATABASE_FILE, StoreError};
use rusqlite::{Connection, params};

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

fn engine() -> Principal {
    Principal::system("engine", [])
}

fn start(test: &TestDatabase, version: &str, webview: Option<&str>) -> StartedRun {
    let start = RunStart {
        app_version: version.into(),
        os_text: "Synthetic OS 1.0".into(),
        webview_text: webview.map(str::to_owned),
    };
    write(test, &engine(), move |ctx| {
        lis_ops::runs::start_run(ctx, &start)
    })
    .unwrap()
}

fn stop(test: &TestDatabase, run: AppRunId) -> Result<(), RuntimeError> {
    write(test, &engine(), move |ctx| {
        lis_ops::runs::stop_run(ctx, run, StopKind::Clean)
    })
}

fn connection(test: &TestDatabase) -> Connection {
    Connection::open(test.data_dir().join(DATABASE_FILE)).unwrap()
}

fn audit(test: &TestDatabase, action: &str) -> Vec<(String, Option<String>, String)> {
    connection(test)
        .prepare(
            "SELECT stream, reason, details FROM audit_events WHERE action = ?1 ORDER BY stream, seq",
        )
        .unwrap()
        .query_map([action], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn is_denied(error: &RuntimeError) -> bool {
    matches!(error, RuntimeError::Store(StoreError::Denied(_)))
}

#[test]
fn starts_record_unclean_stops_gaps_and_changes() {
    let test = open();
    let first = start(&test, "0.4.0", Some("WebKit 1"));
    assert_eq!(
        (
            first.previous_unclean,
            first.gap_seconds,
            first.version_changed
        ),
        (false, None, false)
    );
    test.clock.set(at(30));
    stop(&test, first.run_id).unwrap();
    test.clock.set(at(30 + 600));
    let second = start(&test, "0.4.0", Some("WebKit 1"));
    assert_eq!(
        (second.previous_unclean, second.gap_seconds),
        (false, Some(600))
    );
    assert!(!second.version_changed && !second.webview_changed);

    test.clock.set(at(700));
    write(&test, &engine(), move |ctx| {
        lis_ops::runs::heartbeat(ctx, second.run_id)
    })
    .unwrap();
    test.clock.set(at(760));
    let third = start(&test, "0.5.0", Some("WebKit 2"));
    assert_eq!(
        (third.previous_unclean, third.gap_seconds),
        (true, Some(60))
    );
    assert!(third.version_changed && third.webview_changed);

    let events = audit(&test, lis_ops::runs::APP_START);
    assert_eq!(events.len(), 3);
    assert!(
        events[2].2.contains("\"previous_unclean\":true"),
        "{:?}",
        events[2]
    );
    assert!(events.iter().all(|(stream, _, _)| stream == "security"));
    assert_eq!(audit(&test, lis_ops::runs::APP_STOP).len(), 1);
}

#[test]
fn a_run_stops_once_and_is_not_seen_after_its_stop() {
    let test = open();
    let run = start(&test, "0.4.0", None).run_id;
    stop(&test, run).unwrap();
    assert!(matches!(stop(&test, run), Err(RuntimeError::RunStopped)));
    let heartbeat = write(&test, &engine(), move |ctx| {
        lis_ops::runs::heartbeat(ctx, run)
    });
    assert!(matches!(heartbeat, Err(RuntimeError::RunStopped)));
    let unknown = stop(&test, AppRunId::generate());
    assert!(matches!(unknown, Err(RuntimeError::NotFound(_))));
    let conn = connection(&test);
    for sql in [
        "UPDATE ops_app_runs SET last_seen_at = '2030-01-01T00:00:00.000000000Z'",
        "UPDATE ops_app_runs SET stop_kind = 'quit_with_warnings'",
        "UPDATE ops_app_runs SET app_version = '9.9.9'",
        "DELETE FROM ops_app_runs",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

fn acknowledge(
    test: &TestDatabase,
    principal: &Principal,
    run_id: AppRunId,
    note: &str,
) -> Result<(), RuntimeError> {
    let input = AcknowledgeGapInput {
        run_id,
        note: note.into(),
    };
    write(test, principal, move |ctx| {
        lis_ops::runs::acknowledge_gap(ctx, input).map(drop)
    })
}

/// Three starts: the first, one after a 60-second gap, one after a 61-second gap.
fn three_starts(test: &TestDatabase) -> [AppRunId; 3] {
    let first = start(test, "0.4.0", None).run_id;
    test.clock.set(at(60));
    let short_gap = start(test, "0.4.0", None).run_id;
    test.clock.set(at(60 + 61));
    let long_gap = start(test, "0.4.0", None).run_id;
    [first, short_gap, long_gap]
}

#[test]
fn only_gaps_longer_than_a_minute_need_acknowledgement_by_analysts() {
    let test = open();
    let [first, short_gap, long_gap] = three_starts(&test);
    let facts = test
        .db
        .read_blocking(move |conn| lis_ops::runs::run_facts(conn, at(121)))
        .unwrap();
    assert_eq!((facts.unacknowledged_gaps, facts.unclean_stops_30d), (1, 2));
    let administrator = principal(&[Capability::ManageConfiguration]);
    let denied = acknowledge(&test, &administrator, long_gap, "checked").unwrap_err();
    assert!(is_denied(&denied));
    let analyst = principal(&[Capability::PerformAnalysis]);
    for run in [first, short_gap] {
        let refused = acknowledge(&test, &analyst, run, "checked");
        assert!(matches!(refused, Err(RuntimeError::NoListeningGap)));
    }
}

#[test]
fn a_listening_gap_is_acknowledged_once_with_a_note_on_the_clinical_stream() {
    let test = open();
    let [_, _, long_gap] = three_starts(&test);
    let analyst = principal(&[Capability::PerformAnalysis]);
    let blank = acknowledge(&test, &analyst, long_gap, "  ");
    assert!(matches!(blank, Err(RuntimeError::Invalid(_))));
    acknowledge(&test, &analyst, long_gap, "Analyzer queue re-sent").unwrap();
    let again = acknowledge(&test, &analyst, long_gap, "again");
    assert!(matches!(again, Err(RuntimeError::GapAlreadyAcknowledged)));

    let events = audit(&test, lis_ops::runs::ACKNOWLEDGE_GAP);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "clinical");
    assert_eq!(events[0].1.as_deref(), Some("Analyzer queue re-sent"));
    let runs = test
        .db
        .read_blocking(|conn| lis_ops::runs::list_app_runs(conn, 10))
        .unwrap();
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].run_id, long_gap);
    assert!(runs[0].acknowledged.is_some() && runs[0].previous_unclean);
}

fn jump(test: &TestDatabase, run: AppRunId, step: ClockStep, listening: bool) {
    write(test, &engine(), move |ctx| {
        lis_ops::health::record_clock_jump(ctx, run, step, listening).map(drop)
    })
    .unwrap();
}

fn unacknowledged_gaps(test: &TestDatabase) -> u32 {
    test.db
        .read_blocking(|conn| lis_ops::runs::run_facts(conn, at(0)))
        .unwrap()
        .unacknowledged_gaps
}

fn run_view(test: &TestDatabase) -> lis_ops::runs::AppRunView {
    test.db
        .read_blocking(|conn| lis_ops::runs::list_app_runs(conn, 1))
        .unwrap()
        .remove(0)
}

// M9, NFR-26: a forward jump while listeners are active is a listening gap that needs
// acknowledgement; one while nothing listens is recorded and audited only.
#[test]
fn a_forward_clock_jump_while_listening_is_a_gap_to_acknowledge() {
    let test = open();
    let run = start(&test, "0.4.0", None).run_id;
    jump(&test, run, ClockStep::Forward { seconds: 300 }, false);
    assert_eq!(unacknowledged_gaps(&test), 0);
    assert!(!run_view(&test).needs_acknowledgement);
    let analyst = principal(&[Capability::PerformAnalysis]);
    assert!(matches!(
        acknowledge(&test, &analyst, run, "checked"),
        Err(RuntimeError::NoListeningGap)
    ));

    jump(&test, run, ClockStep::Forward { seconds: 900 }, true);
    assert_eq!(unacknowledged_gaps(&test), 1);
    let view = run_view(&test);
    assert_eq!(
        (view.clock_gap_seconds, view.needs_acknowledgement),
        (Some(900), true)
    );
    acknowledge(&test, &analyst, run, "Analyzer queue re-sent").unwrap();
    assert_eq!(unacknowledged_gaps(&test), 0);
    let view = run_view(&test);
    assert!(!view.needs_acknowledgement && view.acknowledged.is_some());
    assert!(matches!(
        acknowledge(&test, &analyst, run, "again"),
        Err(RuntimeError::GapAlreadyAcknowledged)
    ));
    let changes = audit(&test, lis_ops::health::CLOCK_CHANGE);
    assert_eq!(changes.len(), 2);
    assert!(
        changes[1].2.contains("\"listening\":true"),
        "{:?}",
        changes[1]
    );
    let acknowledged = audit(&test, lis_ops::runs::ACKNOWLEDGE_GAP);
    assert!(acknowledged[0].2.contains("\"clock_gap_seconds\":900"));
}

// M9, NFR-26: the newest backward jump is read from its record, so the 24-hour clock
// alert does not depend on the process that saw it.
#[test]
fn backward_jumps_are_recorded_for_the_clock_alert() {
    let test = open();
    let run = start(&test, "0.4.0", None).run_id;
    let last = || {
        test.db
            .read_blocking(lis_ops::health::last_backward_jump)
            .unwrap()
    };
    assert_eq!(last(), None);
    jump(&test, run, ClockStep::Forward { seconds: 90 }, true);
    assert_eq!(last(), None);
    test.clock.set(at(30));
    jump(&test, run, ClockStep::Backward { seconds: 120 }, true);
    assert_eq!(last(), Some(at(30)));
    assert_eq!(unacknowledged_gaps(&test), 1, "a backward jump is no gap");
    let conn = connection(&test);
    for sql in [
        "UPDATE ops_clock_jumps SET seconds = 1",
        "UPDATE ops_clock_jumps SET acknowledged_by = 'x', acknowledged_at = detected_at \
         WHERE direction = 'backward'",
        "DELETE FROM ops_clock_jumps",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn unclean_stops_count_for_30_days_and_web_engine_changes_are_dated() {
    let test = open();
    start(&test, "0.4.0", Some("WebKit 1"));
    test.clock.set(at(3600));
    let changed = start(&test, "0.4.0", Some("WebKit 2"));
    let day = 86_400;
    let facts = |seconds| {
        test.db
            .read_blocking(move |conn| lis_ops::runs::run_facts(conn, at(seconds)))
            .unwrap()
    };
    assert_eq!(facts(3600 + 30 * day).unclean_stops_30d, 1);
    assert_eq!(facts(3600 + 30 * day + 60).unclean_stops_30d, 0);
    assert_eq!(facts(3600).webview_changed_at, Some(at(3600)));
    assert_ne!(changed.run_id, AppRunId::generate());
}

/// Inserts a backup run of `kind` and `state` finishing at `finished`, with a verified
/// copy when `verified`, through the backup tables' own transitions.
fn backup_run(test: &TestDatabase, kind: &str, finished: UtcTimestamp, verified: bool) -> RunId {
    let conn = connection(test);
    let (run, target) = (RunId::generate(), RunId::generate().to_string());
    conn.execute(
        "INSERT OR IGNORE INTO ops_targets (id, label, kind, purpose, path, created_by, \
         created_at, revision) VALUES (?1, 'Synthetic target', 'removable', \
         'backup_and_anchors', ?2, 'test', ?3, 1)",
        params![target, format!("/synthetic/{target}"), test_start()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ops_backup_runs (id, kind, state, requested_by, requested_at, started_at, revision) \
         VALUES (?1, ?2, CASE WHEN ?2 = 'scheduled' THEN 'running' ELSE 'requested' END, \
         'test', ?3, CASE WHEN ?2 = 'scheduled' THEN ?3 END, 1)",
        params![run, kind, test_start()],
    )
    .unwrap();
    conn.execute(
        "UPDATE ops_backup_runs SET state = 'running', started_at = ?2, revision = 2 \
         WHERE id = ?1 AND state = 'requested'",
        params![run, test_start()],
    )
    .unwrap();
    let copy_state = if verified { "succeeded" } else { "failed" };
    conn.execute(
        "INSERT INTO ops_backup_copies (run_id, target_id, set_id, state, failure_code, \
         written_bytes, verified_at) VALUES (?1, ?2, CASE WHEN ?3 = 'succeeded' THEN 'set-1' END, \
         ?3, CASE WHEN ?3 = 'failed' THEN 'write_failed' END, 10, \
         CASE WHEN ?3 = 'succeeded' THEN ?4 END)",
        params![run, target, copy_state, finished],
    )
    .unwrap();
    conn.execute(
        "UPDATE ops_backup_runs SET state = 'succeeded', finished_at = ?2, \
         snapshot_sha256 = ?3, revision = revision + 1 WHERE id = ?1",
        params![run, finished, "a".repeat(64)],
    )
    .unwrap();
    run
}

fn approval(version: &str, run: RunId) -> NewUpdateApproval {
    NewUpdateApproval {
        version: version.into(),
        risk_class_text: "Minor, as published".into(),
        release_notes_reviewed: true,
        pre_update_run_id: run,
        notes: "Release notes reviewed against lab SOP".into(),
        package_file_name: "vibe-tox-0.5.0.msi".into(),
        package_sha256: "b".repeat(64),
    }
}

fn approve(
    test: &TestDatabase,
    principal: &Principal,
    approval: NewUpdateApproval,
) -> Result<lis_ops::updates::UpdateApprovalView, RuntimeError> {
    write(test, principal, move |ctx| {
        lis_ops::updates::record_update_approval(ctx, &approval)
    })
}

#[test]
fn an_approval_needs_a_recent_retained_verified_backup() {
    let test = open();
    let configurator = principal(&[Capability::ManageConfiguration]);
    test.clock.set(at(25 * 3600));
    let recent = backup_run(&test, "pre_update", at(3600), true);
    let too_old = backup_run(&test, "pre_update", at(3600 - 1), true);
    let scheduled = backup_run(&test, "scheduled", at(20 * 3600), true);
    let unverified = backup_run(&test, "pre_update", at(20 * 3600), false);
    let future = backup_run(&test, "pre_update", at(25 * 3600 + 1), true);
    let pruned = backup_run(&test, "manual", at(20 * 3600), true);
    connection(&test)
        .execute(
            "UPDATE ops_backup_copies SET pruned_at = ?2 WHERE run_id = ?1",
            params![pruned, at(21 * 3600)],
        )
        .unwrap();

    let denied = approve(
        &test,
        &principal(&[Capability::ApproveConfiguration]),
        approval("0.5.0", recent),
    );
    assert!(is_denied(&denied.unwrap_err()));
    for run in [too_old, scheduled, unverified, future, pruned] {
        let record = lis_ops::backup::get_run(&connection(&test), run).unwrap();
        assert!(!lis_ops::updates::update_approval_eligible(
            &record,
            at(25 * 3600)
        ));
        let refused = approve(&test, &configurator, approval("0.5.0", run));
        assert!(
            matches!(refused, Err(RuntimeError::PreUpdateBackupInvalid)),
            "{run}"
        );
        assert!(insert_approval(&test, run, at(25 * 3600)).is_err());
    }
    assert!(matches!(
        approve(&test, &configurator, approval("0.5.0", RunId::generate())),
        Err(RuntimeError::PreUpdateBackupInvalid)
    ));
    let record = lis_ops::backup::get_run(&connection(&test), recent).unwrap();
    assert!(lis_ops::updates::update_approval_eligible(
        &record,
        at(25 * 3600)
    ));
    let view = approve(&test, &configurator, approval("0.5.0", recent)).unwrap();
    assert_eq!((view.revision, view.installed), (1, false));
    let twice = approve(&test, &configurator, approval("0.5.0", recent));
    assert!(matches!(twice, Err(RuntimeError::VersionAlreadyApproved)));
    let event = &audit(&test, lis_ops::updates::APPROVE_UPDATE)[0];
    assert_eq!(
        event.1.as_deref(),
        Some("Release notes reviewed against lab SOP")
    );
    assert!(event.2.contains(&"b".repeat(64)));
}

fn insert_approval(test: &TestDatabase, run: RunId, now: UtcTimestamp) -> rusqlite::Result<usize> {
    connection(test).execute(
        "INSERT INTO ops_update_approvals (id, version, package_file_name, package_sha256, \
         risk_class_text, pre_update_run_id, approved_by, approved_at, revision) \
         VALUES (?1, '0.6.0', 'p.msi', ?2, 'Minor', ?3, 'x', ?4, 1)",
        params![RunId::generate(), "c".repeat(64), run, now],
    )
}

#[test]
fn a_recent_manual_backup_can_support_an_audited_update_approval() {
    let test = open();
    test.clock.set(at(25 * 3600));
    let run = backup_run(&test, "manual", at(3600), true);
    let mut record = lis_ops::backup::get_run(&connection(&test), run).unwrap();
    assert!(lis_ops::updates::update_approval_eligible(
        &record,
        at(25 * 3600)
    ));
    assert!(!lis_ops::updates::update_approval_eligible(
        &record,
        at(25 * 3600 + 1)
    ));
    record.state = lis_ops::backup::RunState::Running;
    assert!(!lis_ops::updates::update_approval_eligible(
        &record,
        at(25 * 3600)
    ));

    let view = approve(
        &test,
        &principal(&[Capability::ManageConfiguration]),
        approval("0.5.0", run),
    )
    .unwrap();
    assert_eq!(view.pre_update_run_id, run);
    let events = audit(&test, lis_ops::updates::APPROVE_UPDATE);
    assert_eq!(events.len(), 1);
    assert!(events[0].2.contains(&run.to_string()));
    assert!(insert_approval(&test, run, at(25 * 3600)).is_ok());
}

#[test]
fn manual_backup_migration_preserves_existing_approval_evidence() {
    let legacy_runtime = lis_core::ModuleSchema {
        migrations: &lis_ops::RUNTIME_MIGRATIONS[..4],
        ..lis_ops::RUNTIME_SCHEMA
    };
    let test = TestDatabase::open(&[lis_ops::SCHEMA, legacy_runtime]).unwrap();
    let pre_update = backup_run(&test, "pre_update", test_start(), true);
    let manual = backup_run(&test, "manual", test_start(), true);
    let actor = principal(&[Capability::ManageConfiguration]);
    let original = approve(&test, &actor, approval("0.5.0", pre_update)).unwrap();
    assert!(insert_approval(&test, manual, test_start()).is_err());

    connection(&test)
        .execute_batch(lis_ops::RUNTIME_MIGRATIONS[4].sql)
        .unwrap();
    let added = approve(&test, &actor, approval("0.6.0", manual)).unwrap();
    assert_eq!(added.pre_update_run_id, manual);
    let saved: (String, i64) = connection(&test)
        .query_row(
            "SELECT pre_update_run_id, revision FROM ops_update_approvals WHERE id = ?1",
            [original.approval_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(saved, (pre_update.to_string(), original.revision));
    assert_eq!(audit(&test, lis_ops::updates::APPROVE_UPDATE).len(), 2);
}

#[test]
fn approval_input_is_validated() {
    let test = open();
    let configurator = principal(&[Capability::ManageConfiguration]);
    let run = backup_run(&test, "pre_update", test_start(), true);
    let fields = |change: fn(&mut NewUpdateApproval)| {
        let mut input = approval("0.5.0", run);
        change(&mut input);
        let error = approve(&test, &configurator, input).unwrap_err();
        error
            .field_errors()
            .iter()
            .map(|error| error.field.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(fields(|a| a.version = "1.4".into()), ["version"]);
    assert_eq!(fields(|a| a.version = "01.4.0".into()), ["version"]);
    assert_eq!(
        fields(|a| a.release_notes_reviewed = false),
        ["releaseNotesReviewed"]
    );
    assert_eq!(fields(|a| a.package_sha256 = "B".repeat(64)), ["package"]);
    assert_eq!(fields(|a| a.notes = " ".into()), ["notes"]);
    assert_eq!(
        fields(|a| a.risk_class_text = String::new()),
        ["riskClassText"]
    );
    for version in ["1.4.0-rc.1", "1.4.0+build.7", "1.4.0-0.alpha-2+x"] {
        let mut input = approval(version, run);
        input.package_file_name = format!("{version}.msi");
        approve(&test, &configurator, input).unwrap();
    }
}

fn withdraw(
    test: &TestDatabase,
    view: &lis_ops::updates::UpdateApprovalView,
    revision: i64,
) -> Result<lis_ops::updates::UpdateApprovalView, RuntimeError> {
    let input = WithdrawUpdateApprovalInput {
        approval_id: view.approval_id,
        expected_revision: revision,
        reason: "Vendor recalled the package".into(),
    };
    write(
        test,
        &principal(&[Capability::ManageConfiguration]),
        move |ctx| lis_ops::updates::withdraw_update_approval(ctx, input),
    )
}

#[test]
fn approvals_are_withdrawn_once_and_only_before_installation() {
    let test = open();
    let configurator = principal(&[Capability::ManageConfiguration]);
    start(&test, "0.4.0", None);
    let run = backup_run(&test, "pre_update", test_start(), true);
    let pending = approve(&test, &configurator, approval("0.5.0", run)).unwrap();
    assert!(matches!(
        withdraw(&test, &pending, 2),
        Err(RuntimeError::RevisionConflict(_))
    ));
    let withdrawn = withdraw(&test, &pending, 1).unwrap();
    assert!(withdrawn.withdrawn.is_some() && withdrawn.revision == 2);
    assert!(matches!(
        withdraw(&test, &withdrawn, 2),
        Err(RuntimeError::ApprovalWithdrawn)
    ));

    let installed = approve(&test, &configurator, approval("0.5.0", run)).unwrap();
    test.clock.set(at(60));
    start(&test, "0.5.0", None);
    assert!(matches!(
        withdraw(&test, &installed, 1),
        Err(RuntimeError::ApprovalInstalled)
    ));
    let approvals = test
        .db
        .read_blocking(lis_ops::updates::list_update_approvals)
        .unwrap();
    assert_eq!(approvals.len(), 2);
    assert!(approvals.iter().any(|view| view.installed));
}

fn accept(test: &TestDatabase, version: &str) -> Result<VersionStatus, RuntimeError> {
    let input = AcceptVersionInput {
        version: version.into(),
        note: "Smoke-tested before clinical use".into(),
    };
    write(
        test,
        &principal(&[Capability::ApproveConfiguration]),
        move |ctx| lis_ops::updates::accept_installed_version(ctx, input).map(|view| view.status),
    )
}

fn statuses(test: &TestDatabase) -> Vec<(String, VersionStatus)> {
    test.db
        .read_blocking(lis_ops::updates::list_installed_versions)
        .unwrap()
        .into_iter()
        .map(|view| (view.version, view.status))
        .collect()
}

fn facts(test: &TestDatabase) -> (bool, bool) {
    let facts = test
        .db
        .read_blocking(lis_ops::updates::version_facts)
        .unwrap();
    (facts.started_without_approval, facts.awaiting_acceptance)
}

#[test]
fn the_first_version_needs_acceptance_only() {
    let test = open();
    assert_eq!(facts(&test), (false, false));
    start(&test, "0.4.0", None);
    assert_eq!(
        statuses(&test),
        [("0.4.0".into(), VersionStatus::AwaitingAcceptance)]
    );
    assert_eq!(facts(&test), (false, true));
    let denied = write(
        &test,
        &principal(&[Capability::ManageConfiguration]),
        |ctx| {
            lis_ops::updates::accept_installed_version(
                ctx,
                AcceptVersionInput {
                    version: "0.4.0".into(),
                    note: "x".into(),
                },
            )
            .map(drop)
        },
    );
    assert!(is_denied(&denied.unwrap_err()));
    assert_eq!(accept(&test, "0.4.0").unwrap(), VersionStatus::Accepted);
    assert!(matches!(
        accept(&test, "0.4.0"),
        Err(RuntimeError::VersionAlreadyAccepted)
    ));
    assert_eq!(facts(&test), (false, false));
}

/// 0.4.0 accepted, then 0.5.0 started at minute 1 without an approval.
fn unapproved_start() -> TestDatabase {
    let test = open();
    start(&test, "0.4.0", None);
    accept(&test, "0.4.0").unwrap();
    test.clock.set(at(60));
    start(&test, "0.5.0", None);
    test
}

#[test]
fn a_version_started_without_approval_alerts_and_cannot_be_accepted() {
    let test = unapproved_start();
    assert_eq!(facts(&test), (true, true));
    assert!(matches!(
        accept(&test, "0.5.0"),
        Err(RuntimeError::VersionNotApproved)
    ));
    assert!(matches!(
        accept(&test, "0.4.0"),
        Err(RuntimeError::VersionNotRunning)
    ));
    assert_eq!(
        statuses(&test),
        [
            ("0.5.0".into(), VersionStatus::Unapproved),
            ("0.4.0".into(), VersionStatus::Accepted)
        ]
    );
}

// M2, NFR-24: an approval recorded after the start does not regularize it; acceptance
// needs a start under the approval, and the alert stays until the version is accepted.
#[test]
fn a_late_approval_needs_a_start_under_it_before_acceptance() {
    let test = unapproved_start();
    test.clock.set(at(120));
    let run = backup_run(&test, "pre_update", at(90), true);
    approve(
        &test,
        &principal(&[Capability::ManageConfiguration]),
        approval("0.5.0", run),
    )
    .unwrap();
    assert!(matches!(
        accept(&test, "0.5.0"),
        Err(RuntimeError::VersionNotStartedSinceApproval)
    ));
    assert_eq!(
        statuses(&test)[0],
        ("0.5.0".into(), VersionStatus::Unapproved)
    );
    assert_eq!(facts(&test), (true, true));

    test.clock.set(at(180));
    start(&test, "0.5.0", None);
    assert_eq!(
        statuses(&test)[0],
        ("0.5.0".into(), VersionStatus::AwaitingAcceptance)
    );
    assert_eq!(facts(&test), (true, true));
    assert_eq!(accept(&test, "0.5.0").unwrap(), VersionStatus::Accepted);
    assert_eq!(facts(&test), (false, false));
}

#[test]
fn an_approved_version_awaits_acceptance_and_old_versions_are_previous() {
    let test = open();
    start(&test, "0.4.0", None);
    let run = backup_run(&test, "pre_update", test_start(), true);
    approve(
        &test,
        &principal(&[Capability::ManageConfiguration]),
        approval("0.5.0", run),
    )
    .unwrap();
    test.clock.set(at(60));
    start(&test, "0.5.0", None);
    assert_eq!(facts(&test), (false, true));
    assert_eq!(
        statuses(&test),
        [
            ("0.5.0".into(), VersionStatus::AwaitingAcceptance),
            ("0.4.0".into(), VersionStatus::Previous)
        ]
    );
    let events = audit(&test, lis_ops::updates::ACCEPT_VERSION);
    assert!(events.is_empty());
    accept(&test, "0.5.0").unwrap();
    let events = audit(&test, lis_ops::updates::ACCEPT_VERSION);
    assert!(events[0].2.contains("approval_id"), "{:?}", events[0]);
}

#[test]
fn app_history_pages_cover_equal_times_and_rollback_without_hiding_old_gaps() {
    let test = open();
    let first = start(&test, "0.4.0", None).run_id;
    jump(&test, first, ClockStep::Forward { seconds: 900 }, true);
    let mut ids = vec![first];
    for _ in 0..204 {
        ids.push(start(&test, "0.4.0", None).run_id);
    }
    test.clock
        .set(UtcTimestamp::parse("2026-01-02T03:00:00.000000000Z").unwrap());
    ids.push(start(&test, "0.4.0", None).run_id);
    let mut found = Vec::new();
    let mut before = None;
    loop {
        let page = test
            .db
            .read_blocking(move |conn| lis_ops::runs::list_app_runs_page(conn, 50, before))
            .unwrap();
        found.extend(page.iter().map(|run| run.run_id));
        if page.len() < 50 {
            break;
        }
        before = page.last().map(|run| run.run_id);
    }
    found.sort();
    let count = found.len();
    found.dedup();
    ids.sort();
    assert_eq!(found, ids);
    assert_eq!(count, ids.len(), "no overlap between pages");
    let gaps = test
        .db
        .read_blocking(lis_ops::runs::pending_listening_gaps)
        .unwrap();
    assert!(gaps.iter().any(|run| run.run_id == first));
    let selected = test
        .db
        .read_blocking(move |conn| lis_ops::runs::get_app_run(conn, first))
        .unwrap();
    assert!(selected.needs_acknowledgement);
    let missing = test
        .db
        .read_blocking(|conn| lis_ops::runs::get_app_run(conn, AppRunId::generate()));
    assert!(matches!(missing, Err(RuntimeError::NotFound("app run"))));
    acknowledge(
        &test,
        &principal(&[Capability::PerformAnalysis]),
        first,
        "Synthetic analyzer reconciliation complete",
    )
    .unwrap();
    assert!(
        test.db
            .read_blocking(lis_ops::runs::pending_listening_gaps)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn app_history_rejects_a_missing_cursor_instead_of_claiming_end_of_history() {
    let test = open();
    start(&test, "0.4.0", None);
    let result = test.db.read_blocking(|conn| {
        lis_ops::runs::list_app_runs_page(conn, 50, Some(AppRunId::generate()))
    });
    assert!(matches!(
        result,
        Err(RuntimeError::NotFound("app run cursor"))
    ));
}

fn begin_install(
    test: &TestDatabase,
    principal: &Principal,
    version: &'static str,
    sha256: String,
) -> Result<lis_ops::updates::UpdateApprovalId, RuntimeError> {
    write(test, principal, move |ctx| {
        lis_ops::updates::begin_update_install(ctx, version, &sha256)
    })
}

#[test]
fn an_install_needs_the_approved_package_and_a_still_recent_backup() {
    let test = open();
    let configurator = principal(&[Capability::ManageConfiguration]);
    start(&test, "0.4.0", None);
    let run = backup_run(&test, "pre_update", test_start(), true);

    let unapproved = begin_install(&test, &configurator, "0.5.0", "b".repeat(64));
    assert!(matches!(unapproved, Err(RuntimeError::InstallNotApproved)));
    let view = approve(&test, &configurator, approval("0.5.0", run)).unwrap();

    let denied = begin_install(
        &test,
        &principal(&[Capability::ApproveConfiguration]),
        "0.5.0",
        "b".repeat(64),
    );
    assert!(is_denied(&denied.unwrap_err()));
    let other = begin_install(&test, &configurator, "0.5.0", "c".repeat(64));
    assert!(matches!(other, Err(RuntimeError::PackageNotApproved)));

    let allowed = begin_install(&test, &configurator, "0.5.0", "b".repeat(64)).unwrap();
    assert_eq!(allowed, view.approval_id);
    let event = &audit(&test, lis_ops::updates::INSTALL_UPDATE)[0];
    assert_eq!(event.0, "security");
    assert!(event.2.contains(&"b".repeat(64)));

    test.clock.set(at(24 * 3600 + 1));
    let stale = begin_install(&test, &configurator, "0.5.0", "b".repeat(64));
    assert!(matches!(stale, Err(RuntimeError::PreUpdateBackupInvalid)));

    test.clock.set(at(60));
    start(&test, "0.5.0", None);
    let installed = begin_install(&test, &configurator, "0.5.0", "b".repeat(64));
    assert!(matches!(installed, Err(RuntimeError::ApprovalInstalled)));
}

#[test]
fn a_failed_install_is_audited_against_its_approval() {
    let test = open();
    let configurator = principal(&[Capability::ManageConfiguration]);
    let run = backup_run(&test, "pre_update", test_start(), true);
    let view = approve(&test, &configurator, approval("0.5.0", run)).unwrap();
    let id = view.approval_id;
    write(&test, &configurator, move |ctx| {
        lis_ops::updates::record_update_install_failure(
            ctx,
            id,
            lis_ops::updates::InstallFailure::Installer,
        )
    })
    .unwrap();
    let events = audit(&test, lis_ops::updates::INSTALL_UPDATE);
    assert_eq!(events.len(), 1);
    assert!(events[0].2.contains("installer_failed"));
}

#[test]
fn versions_compare_by_semantic_version_precedence() {
    use std::cmp::Ordering::{Equal, Greater, Less};

    use lis_ops::updates::version_precedence;

    let ordered = [
        "0.1.1",
        "0.1.2-nightly.20261026.18",
        "0.1.2-nightly.20261027.9",
        "0.1.2-nightly.20261027.10",
        "0.1.2",
        "0.2.0",
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-beta",
        "1.0.0",
    ];
    for pair in ordered.windows(2) {
        assert_eq!(version_precedence(pair[0], pair[1]), Some(Less), "{pair:?}");
        assert_eq!(
            version_precedence(pair[1], pair[0]),
            Some(Greater),
            "{pair:?}"
        );
    }
    assert_eq!(version_precedence("1.0.0+build.1", "1.0.0"), Some(Equal));
    assert_eq!(version_precedence("1.0", "1.0.0"), None);
    assert_eq!(version_precedence("1.0.0", "v1.0.1"), None);
}
