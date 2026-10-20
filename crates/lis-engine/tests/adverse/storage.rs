//! Damaged and unwritable storage (NFR-01, NFR-04, REQ-OPS-007/008; VAL-38, VAL-39):
//! a database file damaged while the app was closed, a snapshot that fails its
//! integrity check, and a backup target that stops accepting writes. Expected outcomes:
//! `docs/planning/m4-design-brief.md` §3.1, §3.5 and §6.4.

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lis_auth::DataKey;
use lis_core::FixedClock;
use lis_engine::{
    AddBackupTargetInput, BackupRunKind, BackupRunView, Engine, EngineConfig, EngineError,
    HealthState, KeyProvider, ListBackupRunsInput, PrinterDestinations, RequestBackupInput,
    TargetId, TargetInput, TargetKind, TargetPurpose,
};
use lis_store::DATABASE_FILE;
use tempfile::TempDir;

use crate::fixture::{TestEngine, start_time};

/// An engine on `dir` with the scenario fixture's settings.
pub(crate) async fn start(dir: &Path, clock: &Arc<FixedClock>) -> Result<Engine, EngineError> {
    Engine::start(EngineConfig {
        data_dir: dir.to_owned(),
        clock: clock.clone(),
        keys: KeyProvider::Fixed(DataKey::from_bytes([7; 32])),
        printer_destinations: PrinterDestinations::LabNetworkAndLoopback,
        webview: None,
        outbox_handlers: Vec::new(),
        installation_kind: lis_engine::InstallationKind::Laboratory,
    })
    .await
}

/// M4 brief §6.4: a database file whose header was overwritten while the app was closed
/// stops the start with `database_unreadable`, the code the startup-failure screen
/// explains, and the damaged file is left as it was for recovery: the app never starts
/// on, migrates or replaces it.
#[tokio::test]
async fn a_database_damaged_while_closed_is_refused_at_start_and_left_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(FixedClock::new(start_time()));
    let engine = start(dir.path(), &clock).await.unwrap();
    engine.shutdown().await;
    drop(engine);
    let file = dir.path().join(DATABASE_FILE);
    // Damage lands in the main file: fold the write-ahead log into it first, or SQLite
    // would read the intact first page from the log.
    let checkpoint = rusqlite::Connection::open(&file).unwrap();
    checkpoint
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(checkpoint);
    let mut damage = OpenOptions::new().write(true).open(&file).unwrap();
    damage.seek(SeekFrom::Start(0)).unwrap();
    damage.write_all(b"SYNTHETIC DAMAGE").unwrap();
    damage.sync_all().unwrap();
    let damaged = fs::read(&file).unwrap();

    let mut refused = None;
    for _ in 0..200 {
        match start(dir.path(), &clock).await {
            Err(error) if error.code() == "already_open" => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => {
                refused = Some(error);
                break;
            }
            Ok(_) => panic!("the app started on a damaged database"),
        }
    }

    assert_eq!(refused.unwrap().code(), "database_unreadable");
    assert_eq!(fs::read(&file).unwrap(), damaged);
}

/// A signed-in administrator, a folder backup target and one verified backup.
struct Backups {
    t: TestEngine,
    target_id: TargetId,
    folder: TempDir,
}

impl Backups {
    async fn start() -> Self {
        let t = TestEngine::start().await;
        t.admin_session().await;
        let folder = tempfile::tempdir().unwrap();
        let target = t
            .engine
            .add_backup_target(
                AddBackupTargetInput {
                    label: "Synthetic backup folder".to_owned(),
                    kind: TargetKind::LocalDisk,
                    purpose: TargetPurpose::BackupAndAnchors,
                },
                Some(folder.path().to_owned()),
            )
            .await
            .unwrap()
            .unwrap();
        let backups = Self {
            t,
            target_id: target.target_id,
            folder,
        };
        assert_eq!(backups.back_up().await.state, "succeeded");
        backups
    }

    /// One manual backup through a job-runner pass; returns its run.
    async fn back_up(&self) -> BackupRunView {
        let e = &self.t.engine;
        e.request_backup(RequestBackupInput {
            kind: BackupRunKind::Manual,
        })
        .await
        .unwrap();
        e.run_operations_pass().await;
        let input = ListBackupRunsInput {
            before_run_id: None,
            run_id: None,
            limit: 1,
            before_requested_at: None,
        };
        e.list_backup_runs(input).await.unwrap().remove(0)
    }

    async fn sets(&self) -> usize {
        let input = TargetInput {
            target_id: self.target_id,
        };
        self.t.engine.list_backup_sets(input).await.unwrap().len()
    }

    /// `(check, state, summary)` of every check not ok.
    async fn attention(&self) -> Vec<(&'static str, HealthState, String)> {
        let health = self.t.engine.get_host_health().await.unwrap();
        health
            .checks
            .into_iter()
            .filter(|check| check.state != HealthState::Ok)
            .map(|check| (check.check.as_str(), check.state, check.summary))
            .collect()
    }
}

/// M4 brief §3.1 step 4: when the snapshot fails `PRAGMA integrity_check`, the run fails
/// with `snapshot_not_intact`, nothing is copied, and the database-damage alert is
/// raised. The damage is simulated by a row written past its table's CHECK constraint,
/// which `integrity_check` reports, as it would report damaged content.
#[tokio::test]
async fn a_snapshot_that_fails_its_integrity_check_is_never_copied_and_raises_an_alert() {
    let backups = Backups::start().await;
    backups
        .t
        .conn()
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA ignore_check_constraints = ON;
             INSERT INTO res_quarantine_items
                 (id, import_attempt_id, record_path, reason, state, revision)
             VALUES ('synthetic-damage', 'synthetic-attempt', 'P1.O1.R1',
                     'synthetic_damage', 'open', 1);",
        )
        .unwrap();

    let run = backups.back_up().await;

    assert_eq!(run.state, "failed");
    assert_eq!(run.failure_code.as_deref(), Some("snapshot_not_intact"));
    assert_eq!(
        backups.sets().await,
        1,
        "only the earlier verified set exists"
    );
    let attention = backups.attention().await;
    let damage_alert = attention.iter().any(|(_, state, summary)| {
        *state == HealthState::Alert && summary.to_lowercase().contains("damage")
    });
    assert!(damage_alert, "no database-damage alert: {attention:?}");
}

/// M4 brief §3.1 step 8 and §3.5: a target that stops accepting writes fails each run
/// with no verified copy, keeps the earlier set, and after three failures in a row the
/// backup-failure signal needs attention.
#[cfg(unix)]
#[tokio::test]
async fn an_unwritable_target_fails_each_run_visibly_and_counts_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let backups = Backups::start().await;
    let root: PathBuf = backups.folder.path().join("VibeToxLIS");
    let set_folder = root.join("sets");
    for folder in [&root, &set_folder] {
        fs::set_permissions(folder, fs::Permissions::from_mode(0o555)).unwrap();
    }

    let mut runs = Vec::new();
    for _ in 0..3 {
        backups.t.advance(60);
        runs.push(backups.back_up().await);
    }
    let attention = backups.attention().await;
    for folder in [&root, &set_folder] {
        fs::set_permissions(folder, fs::Permissions::from_mode(0o755)).unwrap();
    }

    for run in &runs {
        assert_eq!(run.state, "failed", "{run:?}");
        assert!(run.copies.iter().all(|copy| copy.verified_at.is_none()));
    }
    assert_eq!(
        backups.sets().await,
        1,
        "only the earlier verified set exists"
    );
    let failure_signal = attention
        .iter()
        .any(|(_, _, summary)| summary.to_lowercase().contains("fail"));
    assert!(failure_signal, "no backup-failure signal: {attention:?}");
}
