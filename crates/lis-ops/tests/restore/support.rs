//! The shared fixture: a live database with the operations schemas and a synthetic
//! module holding artifact references, folder targets in a temporary folder, and real
//! backups and anchor exports. Nothing is stubbed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_auth::DataKey;
use lis_core::{
    Capability, Clock, CorrelationId, Migration, ModuleSchema, Principal, UtcTimestamp,
};
use lis_ops::backup::{self, BackupError, RunKind, RunState};
use lis_ops::format::{BackupKey, Content, decrypt, encrypt};
use lis_ops::restore::{
    ApplyOutcome, RESTORE_COMPONENT, RestoreFloors, RestoreId, RestoreTestJob, RestoreTestReport,
    StageInput, StagedRestore, applied_restore, apply_staged, confirm_restore,
    finish_recorded_restore, record_applied_restore, request_restore_test, restore_staging_dir,
    run_restore_test, stage,
};
use lis_ops::targets::{self, NewTarget, Target, TargetKind, TargetPurpose};
use lis_store::testing::{TestDatabase, principal};
use lis_store::{Database, StoreError};
use tempfile::TempDir;

/// A synthetic module whose one column holds artifact digests, as reports do.
pub(crate) const DEMO: ModuleSchema = ModuleSchema {
    module: "demo",
    table_prefix: "demo_",
    migrations: &[Migration {
        version: 1,
        name: "demo_reports",
        sql: "CREATE TABLE demo_reports (id INTEGER PRIMARY KEY, artifact_sha256 TEXT NOT NULL) STRICT;",
    }],
};
/// The live database's modules, in crate-map order.
pub(crate) const MODULES: [ModuleSchema; 3] = [lis_ops::SCHEMA, lis_ops::ANCHORS_SCHEMA, DEMO];
/// The columns holding artifact digests.
pub(crate) const ARTIFACT_COLUMNS: &[(&str, &str)] = &[("demo_reports", "artifact_sha256")];

pub(crate) fn installation_key() -> DataKey {
    DataKey::from_bytes([7; 32])
}

pub(crate) fn admin() -> Principal {
    principal(&[
        Capability::BackupRestore,
        Capability::Export,
        Capability::ManageQuality,
    ])
}

pub(crate) fn system(component: &'static str) -> Principal {
    Principal::system(component, [Capability::ManageQuality])
}

pub(crate) struct Fixture {
    pub(crate) test: TestDatabase,
    pub(crate) key: BackupKey,
    pub(crate) folders: TempDir,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let test = TestDatabase::open(&MODULES).unwrap();
        let fixture = Self {
            test,
            key: BackupKey::derive(&installation_key()),
            folders: tempfile::tempdir().unwrap(),
        };
        fixture.write(system("test"), lis_ops::ensure_installation);
        fixture
    }

    pub(crate) fn write<T, E, F>(&self, who: Principal, command: F) -> T
    where
        T: Send + 'static,
        E: From<StoreError> + std::fmt::Debug + Send + 'static,
        F: FnOnce(&mut lis_store::CommandContext<'_>) -> Result<T, E> + Send + 'static,
    {
        self.test
            .db
            .write_blocking(who, CorrelationId::generate(), command)
            .unwrap()
    }

    pub(crate) fn count(&self, sql: &'static str) -> i64 {
        self.test
            .db
            .read_blocking(move |conn| -> Result<i64, StoreError> {
                Ok(conn.query_row(sql, [], |row| row.get(0))?)
            })
            .unwrap()
    }

    pub(crate) fn clock(&self) -> Arc<dyn Clock> {
        self.test.clock.clone()
    }

    pub(crate) fn now(&self) -> UtcTimestamp {
        self.test.clock.now()
    }

    pub(crate) fn advance(&self, minutes: u64) {
        let later = self.now().checked_add(Duration::from_secs(minutes * 60));
        self.test.clock.set(later.unwrap());
    }

    pub(crate) fn target(&self, name: &str, kind: TargetKind) -> Target {
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
        self.write(admin(), move |ctx| {
            targets::add_target(ctx, &data_dir, key_id, &input)
        })
    }

    /// An audited clinical change.
    pub(crate) fn change(&self) {
        self.write(system("test"), |ctx| {
            ctx.audit(AuditEvent::new(
                Stream::Clinical,
                "test.change",
                Outcome::Succeeded,
            ))
        });
    }

    /// Stores an artifact and a report row referencing it; returns the digest.
    pub(crate) fn report(&self, bytes: &[u8]) -> String {
        let digest = self
            .test
            .db
            .artifacts()
            .put(bytes)
            .unwrap()
            .digest
            .to_string();
        self.reference(&digest);
        digest
    }

    /// A report row referencing `digest`, which need not be stored.
    pub(crate) fn reference(&self, digest: &str) {
        let digest = digest.to_owned();
        self.write(system("test"), move |ctx| -> Result<(), StoreError> {
            ctx.tx().execute(
                "INSERT INTO demo_reports (artifact_sha256) VALUES (?1)",
                [&digest],
            )?;
            ctx.audit(AuditEvent::new(
                Stream::Clinical,
                "test.report",
                Outcome::Succeeded,
            ))?;
            Ok(())
        });
    }

    /// A manual backup to every available target; returns the new set's name.
    pub(crate) fn backup(&self, target: &Target) -> String {
        self.write(admin(), |ctx| backup::request_backup(ctx, RunKind::Manual));
        let report = backup::run(
            &self.test.db,
            self.test.data_dir(),
            Some(&self.key),
            &lis_ops::policy::DEFAULT_SETTINGS,
            self.test.clock.as_ref(),
            &AtomicBool::new(false),
        )
        .unwrap()
        .unwrap();
        assert_eq!(report.state, RunState::Succeeded);
        backup::list_sets(&targets::root(target), &self.key)
            .unwrap()
            .pop()
            .unwrap()
            .set_id
    }

    /// A real anchor export pass to every available target, a minute later.
    pub(crate) fn export_anchors(&self) {
        self.advance(1);
        lis_ops::anchors::export_pending(&self.test.db, Principal::system("ops-jobs", [])).unwrap();
    }

    /// Requests and runs a restore test of `target`'s newest set with `key` and `modules`.
    pub(crate) fn restore_test_with(
        &self,
        target: &Target,
        key: &BackupKey,
        modules: &[ModuleSchema],
    ) -> RestoreTestReport {
        let target_id = target.id;
        self.write(admin(), move |ctx| {
            request_restore_test(ctx, target_id, None)
        });
        let job = RestoreTestJob {
            db: &self.test.db,
            data_dir: self.test.data_dir(),
            key: Some(key),
            modules,
            artifact_columns: ARTIFACT_COLUMNS,
            clock: self.clock(),
            cancel: &AtomicBool::new(false),
        };
        run_restore_test(&job).unwrap().unwrap()
    }

    pub(crate) fn restore_test(&self, target: &Target) -> RestoreTestReport {
        self.restore_test_with(target, &self.key, &MODULES)
    }

    /// Stages `target`'s newest set into `scratch` for a restore, keeping its artifacts.
    pub(crate) fn stage_into(&self, target: &Target, scratch: &Path) -> StagedRestore {
        let input = StageInput {
            source_root: &targets::root(target),
            set_id: None,
            key: &self.key,
            scratch_dir: scratch,
            modules: &MODULES,
            artifact_columns: ARTIFACT_COLUMNS,
            anchor_roots: &[],
            keep_artifacts: true,
            clock: self.clock(),
            cancel: &AtomicBool::new(false),
        };
        stage(&input).unwrap()
    }
}

/// Two reports and a change, anchored; a backup; then a change anchored after the set.
pub(crate) fn backed_up() -> (Fixture, Target, String) {
    let f = Fixture::new();
    let target = f.target("usb", TargetKind::Removable);
    f.report(b"synthetic report one");
    f.report(b"synthetic report two");
    f.change();
    f.export_anchors();
    f.advance(1);
    let set_id = f.backup(&target);
    f.change();
    f.export_anchors();
    (f, target, set_id)
}

/// A new installation's data folder, as the setup screen leaves it: a database with
/// the installation row and no users, closed.
pub(crate) fn new_installation(f: &Fixture) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), &MODULES, f.clock()).unwrap();
    db.write_blocking(
        system("test"),
        CorrelationId::generate(),
        lis_ops::ensure_installation,
    )
    .unwrap();
    dir
}

/// A restored installation, opened after the restore was applied and recorded.
pub(crate) struct Restored {
    pub(crate) dir: TempDir,
    pub(crate) db: Database,
    pub(crate) restore_id: RestoreId,
}

/// Stages `target`'s newest set into a new installation, confirms, applies and records
/// it as the engine does at the next start.
pub(crate) fn restore_into_new(f: &Fixture, target: &Target) -> Restored {
    let dir = new_installation(f);
    let staged = f.stage_into(target, &restore_staging_dir(dir.path()));
    assert!(staged.passed(), "{:?}", staged.findings);
    let restore_id = confirm_restore(dir.path(), &staged, f.now()).unwrap();
    let applied = apply_staged(dir.path(), f.now(), |_| false).unwrap();
    assert_eq!(applied, ApplyOutcome::Applied(restore_id));
    let db = Database::open(dir.path(), &MODULES, f.clock()).unwrap();
    record(&db, dir.path());
    finish_recorded_restore(dir.path()).unwrap();
    Restored {
        dir,
        db,
        restore_id,
    }
}

/// Records the applied restore in `data_dir`, with fixed synthetic floors.
pub(crate) fn record(db: &Database, data_dir: &Path) -> RestoreId {
    let marker = applied_restore(data_dir).unwrap().unwrap();
    db.write_blocking(
        system(RESTORE_COMPONENT),
        CorrelationId::generate(),
        move |ctx| {
            record_applied_restore(ctx, &marker, |_| {
                Ok::<_, BackupError>(RestoreFloors {
                    accession: "A26001001".to_owned(),
                    container: "C00005001".to_owned(),
                })
            })
        },
    )
    .unwrap()
}

/// The set folder `set_id` of `target`.
pub(crate) fn set_dir(target: &Target, set_id: &str) -> PathBuf {
    targets::root(target).join("sets").join(set_id)
}

pub(crate) fn decrypted(key: &BackupKey, content: Content, path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    let file = fs::File::open(path).unwrap();
    decrypt(key, content, file, &mut out, &AtomicBool::new(false)).unwrap();
    out
}

pub(crate) fn encrypted(key: &BackupKey, content: Content, path: &Path, plaintext: &[u8]) {
    let mut out = Vec::new();
    encrypt(key, content, plaintext, &mut out, &AtomicBool::new(false)).unwrap();
    fs::write(path, out).unwrap();
}
