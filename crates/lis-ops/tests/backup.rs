//! Backup runs on real temporary folders and SQLite files (REQ-OPS-007, NFR-04, NFR-11,
//! NFR-20, NFR-30; M4 design brief §3.1, §3.3–§3.5). Nothing is stubbed: encryption,
//! digests, snapshots, integrity checks and audit events are the real ones.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_auth::DataKey;
use lis_core::{Capability, CorrelationId, ErrorCode, Principal};
use lis_ops::backup::{
    self, BACKUP, BackupError, Manifest, RunFailure, RunId, RunKind, RunReport, RunState,
};
use lis_ops::format::{BackupKey, Content, decrypt};
use lis_ops::policy::{DEFAULT_SETTINGS, OperationsSettings};
use lis_ops::targets::{self, NewTarget, Target, TargetKind, TargetPurpose};
use lis_store::StoreError;
use lis_store::testing::{TestDatabase, principal};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

struct Fixture {
    test: TestDatabase,
    key: BackupKey,
    folders: TempDir,
}

fn system() -> Principal {
    Principal::system("test", [])
}

fn admin() -> Principal {
    principal(&[Capability::BackupRestore])
}

fn installation_key() -> DataKey {
    DataKey::from_bytes([7; 32])
}

/// The default backup interval, in minutes.
fn interval() -> u64 {
    u64::from(DEFAULT_SETTINGS.backup_interval_minutes)
}

impl Fixture {
    fn new() -> Self {
        let test = TestDatabase::open(&[lis_ops::SCHEMA]).unwrap();
        test.db
            .write_blocking(
                system(),
                CorrelationId::generate(),
                lis_ops::ensure_installation,
            )
            .unwrap();
        Self {
            test,
            key: BackupKey::derive(&installation_key()),
            folders: tempfile::tempdir().unwrap(),
        }
    }

    fn target(&self, name: &str, kind: TargetKind) -> Target {
        let folder = self.folders.path().join(name);
        fs::create_dir_all(&folder).unwrap();
        let data_dir = self.test.data_dir().to_path_buf();
        let key_id = self.key.key_id();
        let input = NewTarget {
            label: format!("{name} label"),
            kind,
            purpose: TargetPurpose::BackupAndAnchors,
            folder,
        };
        self.test
            .db
            .write_blocking(admin(), CorrelationId::generate(), move |ctx| {
                targets::add_target(ctx, &data_dir, key_id, &input)
            })
            .unwrap()
    }

    fn run_with(
        &self,
        settings: &OperationsSettings,
        key: Option<&BackupKey>,
    ) -> Option<RunReport> {
        let cancel = AtomicBool::new(false);
        self.run_cancellable(settings, key, &cancel)
    }

    fn run_cancellable(
        &self,
        settings: &OperationsSettings,
        key: Option<&BackupKey>,
        cancel: &AtomicBool,
    ) -> Option<RunReport> {
        backup::run(
            &self.test.db,
            self.test.data_dir(),
            key,
            settings,
            self.test.clock.as_ref(),
            cancel,
        )
        .unwrap()
    }

    fn run(&self) -> Option<RunReport> {
        self.run_with(&DEFAULT_SETTINGS, Some(&self.key))
    }

    /// Advances past the default backup interval.
    fn advance_past_interval(&self) {
        self.advance(interval() + 1);
    }

    fn advance(&self, minutes: u64) {
        let now = lis_core::Clock::now(self.test.clock.as_ref());
        self.test
            .clock
            .set(now.checked_add(Duration::from_secs(minutes * 60)).unwrap());
    }

    /// An audited clinical change, so the data differs from the newest set.
    fn change(&self) {
        self.test
            .db
            .write_blocking(system(), CorrelationId::generate(), |ctx| {
                ctx.audit(AuditEvent::new(
                    Stream::Clinical,
                    "test.change",
                    Outcome::Succeeded,
                ))
            })
            .unwrap();
    }

    fn request(&self, who: Principal, kind: RunKind) -> Result<RunId, BackupError> {
        self.test
            .db
            .write_blocking(who, CorrelationId::generate(), move |ctx| {
                backup::request_backup(ctx, kind)
            })
    }

    fn is_due(&self) -> bool {
        let now = lis_core::Clock::now(self.test.clock.as_ref());
        self.test
            .db
            .read_blocking(move |conn| backup::is_due(conn, now, &DEFAULT_SETTINGS))
            .unwrap()
    }

    fn put_artifact(&self, bytes: &[u8]) -> String {
        self.test
            .db
            .artifacts()
            .put(bytes)
            .unwrap()
            .digest
            .to_string()
    }

    fn sets(&self, target: &Target) -> Vec<Manifest> {
        backup::list_sets(&targets::root(target), &self.key)
            .unwrap()
            .into_iter()
            .map(|listing| listing.manifest.unwrap())
            .collect()
    }

    fn count(&self, sql: &'static str) -> i64 {
        self.test
            .db
            .read_blocking(move |conn| -> Result<i64, StoreError> {
                Ok(conn.query_row(sql, [], |row| row.get(0))?)
            })
            .unwrap()
    }

    /// The details of each `ops.backup` event, oldest first.
    fn backup_details(&self) -> Vec<serde_json::Value> {
        self.test
            .db
            .read_blocking(|conn| -> Result<Vec<String>, StoreError> {
                let mut statement = conn
                    .prepare("SELECT details FROM audit_events WHERE action = ?1 ORDER BY seq")?;
                let rows = statement
                    .query_map([BACKUP], |row| row.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?;
                Ok(rows)
            })
            .unwrap()
            .iter()
            .map(|details| serde_json::from_str(details).unwrap())
            .collect()
    }
}

fn decrypted(key: &BackupKey, content: Content, path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    decrypt(
        key,
        content,
        fs::File::open(path).unwrap(),
        &mut out,
        &AtomicBool::new(false),
    )
    .unwrap();
    out
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn is_set_id(name: &str) -> bool {
    let bytes = name.as_bytes();
    name.len() == 25
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'T'
        && bytes[9..15].iter().all(u8::is_ascii_digit)
        && &name[15..17] == "Z-"
        && name[17..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

#[test]
fn a_full_run_writes_the_documented_layout_to_two_targets() {
    let f = Fixture::new();
    let report_pdf = f.put_artifact(b"synthetic report bytes");
    let raw_payload = f.put_artifact(b"synthetic analyzer payload");
    let disk = f.target("disk", TargetKind::LocalDisk);
    let usb = f.target("usb", TargetKind::Removable);
    let report = f.run().unwrap();
    assert_eq!(
        (report.kind, report.state, report.failure),
        (RunKind::Scheduled, RunState::Succeeded, None)
    );
    assert_eq!((report.copies_verified, report.copies_failed), (2, 0));
    let artifacts = [
        (report_pdf, b"synthetic report bytes".as_slice()),
        (raw_payload, b"synthetic analyzer payload".as_slice()),
    ];
    for target in [&disk, &usb] {
        check_layout(&f, &targets::root(target), &artifacts);
    }
    assert_eq!(
        f.count("SELECT count(*) FROM ops_backup_copies WHERE state = 'succeeded'"),
        2
    );
    assert_eq!(f.count("SELECT count(*) FROM ops_backup_run_heads"), 3);
    assert!(f.count("SELECT count(*) FROM audit_seals") >= 1);
    assert!(names(&f.test.data_dir().join("ops/staging")).is_empty());
    let details = f.backup_details();
    assert_eq!(details.len(), 1);
    assert_eq!(details[0]["state"], "succeeded");
    assert_eq!(details[0]["copies_verified"], 2);
    assert!(details[0].get("requested_by").is_none());
}

/// `target.json`, one set of exactly two files, and one file per artifact.
fn check_layout(f: &Fixture, root: &Path, artifacts: &[(String, &[u8])]) {
    assert_eq!(names(root), ["artifacts", "sets", "target.json"]);
    let sets = names(&root.join("sets"));
    assert_eq!(sets.len(), 1);
    assert!(is_set_id(&sets[0]), "{}", sets[0]);
    let set = root.join("sets").join(&sets[0]);
    assert_eq!(names(&set), ["database.vtbk", "manifest.vtbk"]);
    let header = fs::read(set.join("database.vtbk")).unwrap();
    assert_eq!(&header[..6], b"VTBK\x01\x01");
    assert_eq!(hex::encode(&header[8..24]), f.key.key_id().to_string());
    check_set(f, &set, &sets[0]);
    let mut expected: Vec<String> = artifacts
        .iter()
        .map(|(digest, _)| format!("{digest}.vtbk"))
        .collect();
    expected.sort();
    assert_eq!(names(&root.join("artifacts")), expected);
    for (digest, bytes) in artifacts {
        let file = root.join("artifacts").join(format!("{digest}.vtbk"));
        assert_eq!(decrypted(&f.key, Content::Artifact, &file), *bytes);
    }
}

/// The manifest describes the decrypted snapshot, which opens as this installation's database.
fn check_set(f: &Fixture, set: &Path, set_id: &str) {
    let manifest = backup::read_manifest(set, &f.key).unwrap();
    assert_eq!(manifest.format, "vibe-tox-set-1");
    assert_eq!(manifest.set_id, set_id);
    assert_eq!(manifest.kind, RunKind::Scheduled);
    assert_eq!(manifest.key_id, f.key.key_id().to_string());
    assert_eq!(manifest.database.integrity, "ok");
    assert_eq!(manifest.artifacts.len(), 2);
    assert!(manifest.artifact_problems.is_empty());
    let streams: Vec<&str> = manifest.heads.iter().map(|h| h.stream.as_str()).collect();
    assert_eq!(streams, ["clinical", "security", "access"]);
    assert!(manifest.seals.iter().any(|seal| seal.stream == "security"));
    assert!(manifest.schema.iter().any(|entry| (
        entry.module.as_str(),
        entry.version,
        entry.name.as_str()
    ) == ("ops", 2, "backup")));
    let database = decrypted(&f.key, Content::Database, &set.join("database.vtbk"));
    assert_eq!(database.len() as u64, manifest.database.bytes);
    assert_eq!(
        hex::encode(Sha256::digest(&database)),
        manifest.database.sha256
    );
    let restored: PathBuf = f.folders.path().join(format!("{set_id}.sqlite3"));
    fs::write(&restored, &database).unwrap();
    let conn = rusqlite::Connection::open(&restored).unwrap();
    let installation: String = conn
        .query_row("SELECT installation_id FROM ops_installation", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(installation, manifest.installation_id.to_string());
    let security_head = &manifest.heads[1];
    let restored_head: i64 = conn
        .query_row(
            "SELECT max(seq) FROM audit_events WHERE stream = 'security'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(restored_head, security_head.seq);
}

#[test]
fn a_run_with_nothing_changed_copies_nothing_until_data_changes() {
    let f = Fixture::new();
    f.target("disk", TargetKind::LocalDisk);
    assert!(f.run().is_some());
    f.change();
    f.advance(interval() - 1);
    assert!(
        !f.is_due(),
        "the interval has not passed since the last attempt"
    );
    assert_eq!(f.run(), None);
    f.advance(1);
    assert!(f.is_due());
    let report = f.run().unwrap();
    assert_eq!(
        (report.kind, report.state),
        (RunKind::Scheduled, RunState::Succeeded)
    );
    f.advance_past_interval();
    assert!(!f.is_due());
    assert_eq!(f.run(), None);
    assert_eq!(f.count("SELECT count(*) FROM ops_backup_runs"), 2);
}

#[test]
fn a_newly_attached_target_is_due_at_once_and_alone() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    f.run().unwrap();
    f.advance(1);
    let usb = f.target("usb", TargetKind::Removable);
    assert!(f.is_due());
    let report = f.run().unwrap();
    assert_eq!(report.copies_verified, 1);
    assert_eq!(f.sets(&usb).len(), 1);
    assert_eq!(f.sets(&disk).len(), 1);
    f.advance_past_interval();
    // Adding the target is a change the disk's set lacks; the completion events are not.
    assert_eq!(f.run().unwrap().copies_verified, 1);
    assert_eq!(f.sets(&disk).len(), 2);
    f.advance_past_interval();
    assert_eq!(f.run(), None);
}

#[test]
fn an_unplugged_target_is_skipped_and_caught_up_when_plugged_in() {
    let f = Fixture::new();
    f.target("disk", TargetKind::LocalDisk);
    let usb = f.target("usb", TargetKind::Removable);
    let away = f.folders.path().join("usb-away");
    fs::rename(&usb.path, &away).unwrap();
    assert_eq!(f.run().unwrap().copies_verified, 1);
    f.advance_past_interval();
    assert_eq!(f.run(), None);
    fs::rename(&away, &usb.path).unwrap();
    assert!(f.is_due());
    assert_eq!(f.run().unwrap().copies_verified, 1);
    assert_eq!(f.sets(&usb).len(), 1);
}

#[test]
fn a_damaged_artifact_and_an_unexpected_entry_are_counted_not_fatal() {
    let f = Fixture::new();
    let good = f.put_artifact(b"synthetic report bytes");
    let damaged = f.put_artifact(b"synthetic payload to damage");
    let store = f.test.data_dir().join("artifacts");
    fs::write(store.join(&damaged), b"synthetic payload to damagX").unwrap();
    fs::write(store.join("copied-in-by-hand.txt"), b"x").unwrap();
    let disk = f.target("disk", TargetKind::LocalDisk);
    let report = f.run().unwrap();
    assert_eq!(
        (report.state, report.artifact_problems),
        (RunState::Succeeded, 2)
    );
    let manifest = &f.sets(&disk)[0];
    let copied: Vec<&str> = manifest
        .artifacts
        .iter()
        .map(|a| a.sha256.as_str())
        .collect();
    assert_eq!(copied, [good.as_str()]);
    let mut problems: Vec<(Option<&str>, &str)> = manifest
        .artifact_problems
        .iter()
        .map(|p| (p.sha256.as_deref(), p.problem.as_str()))
        .collect();
    problems.sort();
    assert_eq!(
        problems,
        [
            (None, "unexpected_entry"),
            (Some(damaged.as_str()), "damaged")
        ]
    );
    assert_eq!(f.count("SELECT artifact_problems FROM ops_backup_runs"), 2);
    assert_eq!(f.backup_details()[0]["artifact_problems"], 2);
}

#[test]
fn an_interrupted_run_leaves_no_listed_set_and_no_copy() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    let cancelled = AtomicBool::new(true);
    let report = f
        .run_cancellable(&DEFAULT_SETTINGS, Some(&f.key), &cancelled)
        .unwrap();
    assert_eq!(report.state, RunState::Interrupted);
    assert!(f.sets(&disk).is_empty());
    assert!(!targets::root(&disk).join("sets").exists());
    assert_eq!(f.count("SELECT count(*) FROM ops_backup_copies"), 0);
    assert_eq!(
        f.count("SELECT count(*) FROM ops_backup_runs WHERE state = 'interrupted'"),
        1
    );
    assert!(names(&f.test.data_dir().join("ops/staging")).is_empty());
    assert_eq!(f.backup_details()[0]["state"], "interrupted");
    assert!(
        f.is_due(),
        "an interrupted run is never counted as verified"
    );
    assert_eq!(f.run().unwrap().state, RunState::Succeeded);
}

#[test]
fn leftovers_of_a_crash_are_never_listed_and_are_cleaned_by_the_next_run() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    let root = targets::root(&disk);
    let half_set = root.join("sets/20260102T030405Z-0000abcd.tmp");
    fs::create_dir_all(&half_set).unwrap();
    fs::write(half_set.join("database.vtbk"), b"partial").unwrap();
    fs::create_dir_all(root.join("artifacts")).unwrap();
    fs::write(root.join("artifacts/abc.vtbk.tmp"), b"partial").unwrap();
    let staging = f.test.data_dir().join("ops/staging");
    fs::create_dir_all(&staging).unwrap();
    fs::write(
        staging.join("0199e2a0-0000-7000-8000-000000000001.sqlite3"),
        b"stale",
    )
    .unwrap();
    assert!(f.sets(&disk).is_empty());
    f.request(admin(), RunKind::Manual).unwrap();
    let abandoned = f
        .test
        .db
        .write_blocking(
            system(),
            CorrelationId::generate(),
            backup::interrupt_abandoned,
        )
        .unwrap();
    assert_eq!(abandoned, 1);
    f.run().unwrap();
    assert_eq!(names(&root.join("sets")).len(), 1);
    assert!(!half_set.exists());
    assert!(!root.join("artifacts/abc.vtbk.tmp").exists());
    assert!(names(&staging).is_empty());
    assert_eq!(
        f.count("SELECT count(*) FROM ops_backup_runs WHERE state = 'interrupted'"),
        1
    );
}

#[test]
fn a_valid_set_copied_under_a_newer_name_is_reported_not_used() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    f.run().unwrap();
    let root = targets::root(&disk);
    let original = names(&root.join("sets")).remove(0);
    let copied = "20991231T235959Z-deadbeef";
    let copied_dir = root.join("sets").join(copied);
    fs::create_dir(&copied_dir).unwrap();
    for name in names(&root.join("sets").join(&original)) {
        fs::copy(
            root.join("sets").join(&original).join(&name),
            copied_dir.join(name),
        )
        .unwrap();
    }

    let listings = backup::list_sets(&root, &f.key).unwrap();
    let renamed = listings
        .iter()
        .find(|listing| listing.set_id == copied)
        .unwrap();
    assert!(
        renamed.manifest.is_err(),
        "the authenticated set ID must bind its folder"
    );
    assert_eq!(
        listings
            .iter()
            .filter(|listing| listing.manifest.is_ok())
            .count(),
        1,
        "the copied set is never usable"
    );
}

#[test]
fn startup_reconciliation_quarantines_a_published_set_without_a_copy_row() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    f.run().unwrap();
    f.test
        .db
        .write_blocking(
            system(),
            CorrelationId::generate(),
            |ctx| -> Result<(), StoreError> {
                ctx.tx().execute_batch(
                    "DROP TRIGGER ops_backup_copies_no_delete; DELETE FROM ops_backup_copies;",
                )?;
                Ok(())
            },
        )
        .unwrap();

    let key = BackupKey::derive(&installation_key());
    let reconciled = f
        .test
        .db
        .read_blocking(move |conn| backup::reconcile_published_sets(conn, &key))
        .unwrap();
    assert_eq!(reconciled.quarantined, 1);
    assert!(f.sets(&disk).is_empty());
    assert_eq!(
        names(&targets::root(&disk).join("quarantine")).len(),
        1,
        "the unrecorded final set is moved aside, not deleted"
    );
}

#[test]
fn pruning_keeps_manual_pre_update_recent_and_foreign_sets() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    let foreign = foreign_set(&f, &disk);
    let settings = OperationsSettings {
        keep_recent: 2,
        keep_daily: 0,
        keep_weekly: 0,
        ..DEFAULT_SETTINGS
    };
    let mut kinds = Vec::new();
    for kind in [
        RunKind::Scheduled,
        RunKind::Manual,
        RunKind::Scheduled,
        RunKind::PreUpdate,
        RunKind::Scheduled,
        RunKind::Scheduled,
        RunKind::Scheduled,
    ] {
        f.change();
        f.advance_past_interval();
        if kind != RunKind::Scheduled {
            f.request(admin(), kind).unwrap();
        }
        let report = f.run_with(&settings, Some(&f.key)).unwrap();
        assert_eq!(report.state, RunState::Succeeded);
        kinds.push(kind);
    }
    let kept: Vec<RunKind> = f
        .sets(&disk)
        .iter()
        .filter(|manifest| manifest.set_id != foreign)
        .map(|manifest| manifest.kind)
        .collect();
    assert_eq!(
        kept,
        [
            RunKind::Manual,
            RunKind::PreUpdate,
            RunKind::Scheduled,
            RunKind::Scheduled
        ]
    );
    assert!(targets::root(&disk).join("sets").join(&foreign).exists());
    assert_eq!(
        f.count("SELECT count(*) FROM ops_backup_copies WHERE pruned_at IS NOT NULL"),
        3
    );
    let pruned: i64 = f
        .backup_details()
        .iter()
        .map(|details| details["sets_pruned"].as_i64().unwrap())
        .sum();
    assert_eq!(pruned, 3);
}

/// Puts a set of another installation, under the same key, into the target's sets.
fn foreign_set(f: &Fixture, ours: &Target) -> String {
    let other = Fixture::new();
    let target = other.target("other", TargetKind::LocalDisk);
    other.run().unwrap();
    let set_id = names(&targets::root(&target).join("sets")).remove(0);
    let from = targets::root(&target).join("sets").join(&set_id);
    let to = targets::root(ours).join("sets").join(&set_id);
    fs::create_dir_all(&to).unwrap();
    for name in names(&from) {
        fs::copy(from.join(&name), to.join(&name)).unwrap();
    }
    let theirs = backup::read_manifest(&from, &other.key).unwrap();
    let installation = f.test.db.read_blocking(lis_ops::installation_id).unwrap();
    assert_ne!(Some(theirs.installation_id), installation);
    set_id
}

#[test]
fn without_the_key_a_due_run_fails_and_writes_nothing() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    let report = f.run_with(&DEFAULT_SETTINGS, None).unwrap();
    assert_eq!(
        (report.state, report.failure),
        (RunState::Failed, Some(RunFailure::KeyUnavailable))
    );
    assert!(!targets::root(&disk).join("sets").exists());
    assert_eq!(f.backup_details()[0]["failure_code"], "key_unavailable");
}

// The engine's job runner calls `run` until it returns `None`: a scheduled run that fails
// before any copy (no key, too little space) must still count as an attempt, or every
// call would start another failing run.
#[test]
fn a_scheduled_run_that_fails_before_copying_waits_for_the_interval() {
    let f = Fixture::new();
    f.target("disk", TargetKind::LocalDisk);
    let failed = f.run_with(&DEFAULT_SETTINGS, None).unwrap();
    assert_eq!(failed.failure, Some(RunFailure::KeyUnavailable));
    assert!(!f.is_due());
    assert_eq!(f.run_with(&DEFAULT_SETTINGS, None), None);
    assert_eq!(f.count("SELECT count(*) FROM ops_backup_runs"), 1);
    f.advance_past_interval();
    assert!(f.is_due());
    assert_eq!(f.run().unwrap().state, RunState::Succeeded);
}

#[test]
fn a_requested_backup_copies_to_every_available_target_even_without_changes() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    f.run().unwrap();
    let user = admin();
    let run_id = f.request(user.clone(), RunKind::PreUpdate).unwrap();
    assert!(matches!(
        f.request(admin(), RunKind::Manual),
        Err(BackupError::RequestQueued)
    ));
    assert!(f.is_due());
    let report = f.run().unwrap();
    assert_eq!(
        (report.run_id, report.kind, report.state),
        (run_id, RunKind::PreUpdate, RunState::Succeeded)
    );
    assert_eq!(f.sets(&disk).len(), 2);
    let details = f.backup_details();
    assert_eq!(details[1]["requested_by"], user.actor().id_text());
    assert_eq!(
        f.count("SELECT count(*) FROM audit_events WHERE action = 'ops.request_backup'"),
        1
    );
}

#[test]
fn a_backup_request_is_refused_without_capability_target_or_valid_kind() {
    let f = Fixture::new();
    assert!(matches!(
        f.request(admin(), RunKind::Manual),
        Err(BackupError::NoBackupTarget)
    ));
    f.target("disk", TargetKind::LocalDisk);
    let denied = f
        .request(
            principal(&[Capability::ManageConfiguration]),
            RunKind::Manual,
        )
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
    assert!(matches!(
        f.request(admin(), RunKind::Scheduled),
        Err(BackupError::Invalid(_))
    ));
    assert_eq!(f.count("SELECT count(*) FROM ops_backup_runs"), 0);
}

#[test]
fn a_requested_run_without_an_available_target_fails() {
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::Removable);
    f.request(admin(), RunKind::Manual).unwrap();
    fs::remove_dir_all(&disk.path).unwrap();
    let report = f.run().unwrap();
    assert_eq!(report.failure, Some(RunFailure::NoTargetAvailable));
}

#[cfg(unix)]
#[test]
fn a_failed_copy_is_recorded_and_the_run_succeeds_with_the_other_target() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let disk = f.target("disk", TargetKind::LocalDisk);
    let share = f.target("share", TargetKind::NetworkShare);
    let sets = targets::root(&share).join("sets");
    fs::create_dir_all(&sets).unwrap();
    fs::set_permissions(&sets, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(sets.join("probe"), b"x").is_ok() {
        return;
    }
    let report = f.run().unwrap();
    fs::set_permissions(&sets, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(report.state, RunState::Succeeded);
    assert_eq!((report.copies_verified, report.copies_failed), (1, 1));
    assert_eq!(f.sets(&disk).len(), 1);
    assert!(f.sets(&share).is_empty());
    assert_eq!(
        f.count("SELECT count(*) FROM ops_backup_copies WHERE failure_code = 'write_failed'"),
        1
    );
}

#[test]
fn run_and_copy_rows_follow_their_state_machines() {
    let f = Fixture::new();
    f.target("disk", TargetKind::LocalDisk);
    f.run().unwrap();
    let refused: Vec<bool> = f
        .test
        .db
        .write_blocking(
            system(),
            CorrelationId::generate(),
            |ctx| -> Result<_, StoreError> {
                Ok([
                    "UPDATE ops_backup_runs SET state = 'running', revision = revision + 1",
                    "DELETE FROM ops_backup_runs",
                    "UPDATE ops_backup_copies SET written_bytes = 0",
                    "DELETE FROM ops_backup_copies",
                    "UPDATE ops_backup_run_heads SET seq = 0",
                    "INSERT INTO ops_backup_runs (id, kind, state, requested_at, revision) \
                 VALUES ('0199e2a0-0000-7000-8000-000000000009', 'manual', 'requested', \
                 '2026-01-02T03:04:05.000000000Z', 1)",
                ]
                .iter()
                .map(|sql| ctx.tx().execute(sql, []).is_err())
                .collect())
            },
        )
        .unwrap();
    assert_eq!(refused, [true; 6]);
}
