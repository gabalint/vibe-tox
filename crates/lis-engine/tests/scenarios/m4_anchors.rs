//! M4 exit scenario 3, anchor tampering (VAL-40 subset; NFR-18, ADR 0008, M4 brief §4,
//! §10), and the exit export read by the independent verifier (NFR-12, REQ-OPS-010).
//! Synthetic data only.
//!
//! The scenarios drive the engine's anchor commands and add targets with
//! `add_backup_target`. In the engine a backup and an exit export run on the job runner and
//! a restore is applied at the next start, so to stay deterministic a [`Station`] stops its
//! engine and runs those steps through `lis_ops` on the same data folder, then starts the
//! engine again. Tampering is done the way a host administrator would: raw SQL with the
//! append-only triggers dropped.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use lis_audit::{Attribution, AuditEvent, Outcome, Stream};
use lis_auth::{
    ChangeInitialPasswordInput, CreateFirstAdministratorInput, CreateUserInput, DataKey, RoleCode,
    SignInInput, SignInStep, VerifyMfaInput,
};
use lis_core::{
    Capability, Clock, CommandError, CorrelationId, ErrorCode, FixedClock, ModuleSchema, Principal,
    UserId,
};
use lis_engine::{
    AddBackupTargetInput, AnchorVerification, Engine, EngineConfig, KeyProvider,
    ListAnchorExportsInput, VerifyAuditChainInput,
};
use lis_ops::backup::{self, RunKind, RunState};
use lis_ops::exit_export::{self, ExitExportJob, ExportState};
use lis_ops::format::BackupKey;
use lis_ops::restore::{
    ApplyOutcome, RESTORE_COMPONENT, RestoreFloors, StageInput, applied_restore, apply_staged,
    confirm_restore, finish_recorded_restore, record_applied_restore, restore_staging_dir, stage,
};
use lis_ops::targets::{self, Target, TargetKind, TargetPurpose};
use lis_store::{DATABASE_FILE, Database, StoreError};
use rusqlite::{Connection, Transaction};
use tempfile::TempDir;

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, WEBVIEW, challenge_id, secret, start_time};
use crate::m2_lab::Role;
use crate::m3_fixture::{Bench, Staff, Variant};

/// The engine's schema modules in crate-map order (`engine.rs` `MODULES`); the store
/// refuses to open the database with any other list.
pub(crate) const MODULES: &[ModuleSchema] = &[
    lis_auth::SCHEMA,
    lis_identity::SCHEMA,
    lis_catalog::SCHEMA,
    lis_interfaces::SCHEMA,
    lis_orders::SCHEMA,
    lis_specimens::SCHEMA,
    lis_analytical::SCHEMA,
    lis_results::SCHEMA,
    lis_reporting::SCHEMA,
    lis_billing::SCHEMA,
    lis_ops::SCHEMA,
    lis_ops::RUNTIME_SCHEMA,
    lis_ops::ANCHORS_SCHEMA,
];
/// Columns holding artifact digests in this schema, for the exit export and staging.
const ARTIFACT_COLUMNS: &[(&str, &str)] = &[
    ("ifc_receipts", "artifact_digest"),
    ("rpt_candidates", "artifact_sha256"),
    ("bil_handoffs", "artifact_sha256"),
];
/// The operations account: administrator and auditor.
pub(crate) const OPS: &str = "ops1";
/// The fixed installation key of every station (synthetic).
pub(crate) const KEY_BYTES: [u8; 32] = [7; 32];

pub(crate) fn installation_key() -> DataKey {
    DataKey::from_bytes(KEY_BYTES)
}

/// A system principal for the offline steps the backup area's commands will take over.
pub(crate) fn operator() -> Principal {
    Principal::system(
        "anchor-scenario",
        [Capability::BackupRestore, Capability::Export],
    )
}

/// One installation: its data folder, a folder for targets and exports, a fixed clock,
/// and either the running engine (with `ops1` signed in) or the database opened offline.
pub(crate) struct Station {
    pub(crate) dir: TempDir,
    pub(crate) folders: TempDir,
    pub(crate) clock: Arc<FixedClock>,
    engine: Option<Engine>,
    offline: Option<Arc<Database>>,
    totp: String,
}

impl Station {
    /// A started engine with the first administrator and `ops1` (administrator and
    /// auditor) signed in.
    pub(crate) async fn new() -> Self {
        let mut station = Self {
            dir: tempfile::tempdir().unwrap(),
            folders: tempfile::tempdir().unwrap(),
            clock: Arc::new(FixedClock::new(start_time())),
            engine: None,
            offline: None,
            totp: String::new(),
        };
        station.engine = Some(station.launch().await);
        station.bootstrap().await;
        station
    }

    pub(crate) fn engine(&self) -> &Engine {
        self.engine.as_ref().expect("the engine is stopped")
    }

    pub(crate) fn advance(&self, seconds: u64) {
        let later = self.clock.now().checked_add(Duration::from_secs(seconds));
        self.clock.set(later.unwrap());
    }

    async fn launch(&self) -> Engine {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let started = Engine::start(EngineConfig {
                data_dir: self.dir.path().to_owned(),
                clock: self.clock.clone(),
                keys: KeyProvider::Fixed(installation_key()),
                printer_destinations: lis_engine::PrinterDestinations::LabNetwork,
                webview: Some(WEBVIEW.to_owned()),
                outbox_handlers: Vec::new(),
                installation_kind: lis_engine::InstallationKind::Laboratory,
            })
            .await;
            match started {
                Ok(engine) => return engine,
                Err(error) => assert!(
                    tokio::time::Instant::now() < deadline,
                    "the engine did not start: {error}"
                ),
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn bootstrap(&mut self) {
        let e = self.engine();
        e.create_first_administrator(CreateFirstAdministratorInput {
            username: ADMIN.into(),
            display_name: "Synthetic Administrator".into(),
            password: secret(PASSWORD),
            one_person: false,
            load_demo_data: None,
        })
        .await
        .unwrap();
        let step = self.sign_in_step(ADMIN, PASSWORD).await;
        self.enrol(step).await;
        e.create_user(CreateUserInput {
            username: OPS.into(),
            display_name: "Synthetic Operations".into(),
            roles: vec![RoleCode::Administrator, RoleCode::Auditor],
            temporary_password: secret(PASSWORD),
            password: secret(PASSWORD).into(),
        })
        .await
        .unwrap();
        let step = self.sign_in_step(OPS, PASSWORD).await;
        let next = e
            .change_initial_password(ChangeInitialPasswordInput {
                challenge_id: challenge_id(&step),
                new_password: secret(NEW_PASSWORD),
            })
            .await
            .unwrap();
        self.totp = self.enrol(next).await;
    }

    async fn sign_in_step(&self, username: &str, password: &str) -> SignInStep {
        self.engine()
            .sign_in(SignInInput {
                username: username.into(),
                password: secret(password),
            })
            .await
            .unwrap()
    }

    /// Completes an enrolment; returns the authenticator secret.
    async fn enrol(&self, step: SignInStep) -> String {
        let SignInStep::MfaEnrollmentRequired { enrollment, .. } = &step else {
            panic!("expected enrolment, got {step:?}");
        };
        let totp = enrollment.secret_base32.clone();
        self.verify(&step, &totp).await;
        totp
    }

    async fn verify(&self, step: &SignInStep, totp: &str) {
        let secret_bytes = totp_rs::Secret::try_from_base32(totp).unwrap();
        let generator = totp_rs::Builder::new()
            .with_secret(secret_bytes)
            .build()
            .unwrap();
        let seconds = u64::try_from(self.clock.now().unix_seconds()).unwrap();
        self.engine()
            .verify_mfa(VerifyMfaInput {
                challenge_id: challenge_id(step),
                code: secret(&generator.generate(seconds).to_string()),
            })
            .await
            .unwrap();
    }

    /// Signs `ops1` in again, half a minute after the last code.
    pub(crate) async fn sign_in(&self) {
        self.advance(30);
        let step = self.sign_in_step(OPS, NEW_PASSWORD).await;
        assert!(matches!(step, SignInStep::MfaRequired { .. }), "{step:?}");
        self.verify(&step, &self.totp).await;
    }

    /// Shuts the engine down and opens the database offline.
    pub(crate) async fn stop(&mut self) {
        let engine = self.engine.take().expect("the engine is stopped");
        engine.shutdown().await;
        drop(engine);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            match Database::open(self.dir.path(), MODULES, self.clock.clone()) {
                Ok(db) => {
                    self.offline = Some(Arc::new(db));
                    return;
                }
                Err(StoreError::AlreadyOpen) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => panic!("the database did not open offline: {error}"),
            }
        }
    }

    /// Closes the offline database, starts the engine and signs `ops1` in.
    pub(crate) async fn start(&mut self) {
        self.offline = None;
        self.engine = Some(self.launch().await);
        self.sign_in().await;
    }

    /// Runs a blocking `lis_ops` step on the offline database.
    pub(crate) async fn offline<T: Send + 'static>(
        &self,
        step: impl FnOnce(&Database, &Path) -> T + Send + 'static,
    ) -> T {
        let db = Arc::clone(self.offline.as_ref().expect("the engine is running"));
        let dir = self.dir.path().to_owned();
        tokio::task::spawn_blocking(move || step(&db, &dir))
            .await
            .unwrap()
    }

    /// Adds a target folder under `folders` through `add_backup_target`.
    pub(crate) async fn add_target(&self, folder: &str, label: &str, kind: TargetKind) -> Target {
        let folder = self.folders.path().join(folder);
        fs::create_dir_all(&folder).unwrap();
        let input = AddBackupTargetInput {
            label: label.to_owned(),
            kind,
            purpose: TargetPurpose::BackupAndAnchors,
        };
        let view = self
            .engine()
            .add_backup_target(input, Some(folder))
            .await
            .unwrap()
            .unwrap();
        targets::active(&self.raw())
            .unwrap()
            .into_iter()
            .find(|target| target.id == view.target_id)
            .unwrap()
    }

    /// `count` synthetic clinical changes, each with `reason`.
    pub(crate) async fn changes(&self, count: usize, reason: &'static str) {
        self.offline(move |db, _| (0..count).for_each(|_| change(db, reason)))
            .await;
    }

    /// One anchor export pass through the engine, a minute later.
    pub(crate) async fn export_anchors(&self) -> lis_engine::AnchorExportReport {
        self.advance(60);
        self.engine().export_anchors_now().await.unwrap()
    }

    pub(crate) async fn verify_targets(&self) -> AnchorVerification {
        self.engine().verify_audit_anchors(None).await.unwrap()
    }

    /// A raw connection, as a host administrator would open the file.
    pub(crate) fn raw(&self) -> Connection {
        Connection::open(self.dir.path().join(DATABASE_FILE)).unwrap()
    }

    /// Runs `edit` with the audit and restore tables' append-only triggers dropped, then
    /// restores them: privileged tampering, which only anchors kept off the host detect.
    pub(crate) fn tamper(&self, edit: impl FnOnce(&Transaction<'_>)) {
        let mut conn = self.raw();
        let tx = conn.transaction().unwrap();
        let triggers: Vec<(String, String)> = tx
            .prepare(
                "SELECT name, sql FROM sqlite_schema WHERE type = 'trigger' \
                 AND tbl_name IN ('audit_events', 'audit_seals', 'ops_restores', \
                 'ops_restore_heads')",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for (name, _) in &triggers {
            tx.execute_batch(&format!("DROP TRIGGER \"{name}\""))
                .unwrap();
        }
        edit(&tx);
        for (_, sql) in &triggers {
            tx.execute_batch(sql).unwrap();
        }
        tx.commit().unwrap();
    }

    /// Moves the engine-less station to another data folder (a restored installation).
    fn move_to(&mut self, dir: TempDir) {
        assert!(self.engine.is_none(), "stop the engine first");
        self.offline = None;
        self.dir = dir;
    }
}

/// Runs one write command on `db` as [`operator`].
pub(crate) fn write<T, E, F>(db: &Database, command: F) -> T
where
    T: Send + 'static,
    E: From<StoreError> + std::fmt::Debug + Send + 'static,
    F: FnOnce(&mut lis_store::CommandContext<'_>) -> Result<T, E> + Send + 'static,
{
    db.write_blocking(operator(), CorrelationId::generate(), command)
        .unwrap()
}

/// One synthetic clinical change with `reason`.
fn change(db: &Database, reason: &'static str) {
    write(db, move |ctx| {
        let event = AuditEvent::new(Stream::Clinical, "test.change", Outcome::Succeeded);
        ctx.audit(event.reason(reason)).map(drop)
    });
}

/// One manual backup to every available target (the backup area's `request_backup` and
/// its job runner).
pub(crate) fn backup_now(db: &Database, dir: &Path, clock: &dyn Clock) {
    write(db, |ctx| backup::request_backup(ctx, RunKind::Manual));
    let key = BackupKey::derive(&installation_key());
    let report = backup::run(
        db,
        dir,
        Some(&key),
        &lis_ops::policy::DEFAULT_SETTINGS,
        clock,
        &AtomicBool::new(false),
    )
    .unwrap()
    .unwrap();
    assert_eq!(report.state, RunState::Succeeded);
}

/// A station with a removable target, six clinical changes and one anchor export.
async fn anchored() -> (Station, Target) {
    let mut station = Station::new().await;
    let target = station
        .add_target("usb", "Synthetic USB", TargetKind::Removable)
        .await;
    station.stop().await;
    station.changes(6, "synthetic change").await;
    station.start().await;
    let report = station.export_anchors().await;
    assert_eq!((report.targets_written, report.targets_failed), (1, 0));
    (station, target)
}

fn has_finding(verification: &AnchorVerification, code: &str, stream: &str) -> bool {
    verification
        .findings
        .iter()
        .any(|finding| finding.code == code && finding.stream.as_deref() == Some(stream))
}

fn chain_intact(verification: &AnchorVerification, stream: &str) -> bool {
    verification
        .chains
        .iter()
        .find(|chain| chain.stream == stream)
        .unwrap()
        .intact
}

#[tokio::test]
async fn untouched_anchors_all_match_and_the_check_is_an_access_event() {
    let (station, _) = anchored().await;
    let verification = station.verify_targets().await;
    assert!(verification.summary.compared >= 2, "{verification:?}");
    assert_eq!(verification.summary.matched, verification.summary.compared);
    assert_eq!(verification.problems(), 0, "{verification:?}");
    let (action, outcome): (String, String) = station
        .raw()
        .query_row(
            "SELECT action, outcome FROM audit_events WHERE stream = 'access' \
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (action.as_str(), outcome.as_str()),
        ("audit.verify_anchors", "succeeded")
    );
    let exports = station
        .engine()
        .list_anchor_exports(ListAnchorExportsInput { limit: 10 })
        .await
        .unwrap();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].target_label, "Synthetic USB");
}

// VAL-40: an event edited in place breaks its chain and every anchor at or after it.
#[tokio::test]
async fn a_row_edited_without_recomputing_the_chain_breaks_it_and_mismatches() {
    let (mut station, _) = anchored().await;
    station.stop().await;
    station.tamper(|tx| {
        let edited = tx
            .execute(
                "UPDATE audit_events SET reason = 'edited' WHERE stream = 'clinical' AND seq = 2",
                [],
            )
            .unwrap();
        assert_eq!(edited, 1);
    });
    station.start().await;
    let verification = station.verify_targets().await;
    assert!(!chain_intact(&verification, "clinical"));
    assert!(
        has_finding(&verification, "anchor_mismatch", "clinical"),
        "{verification:?}"
    );
    assert!(verification.problems() > 0);
    let chain = station
        .engine()
        .verify_audit_chain(VerifyAuditChainInput {
            stream: lis_engine::AuditStream::Clinical,
        })
        .await
        .unwrap();
    assert!(!chain.intact);
}

// VAL-40: an administrator who rewrites events and recomputes every hash and seal leaves
// a locally perfect chain; only the anchors kept off the host show it.
#[tokio::test]
async fn a_row_edited_with_the_chain_recomputed_is_an_anchor_mismatch() {
    let (mut station, _) = anchored().await;
    station.stop().await;
    let now = station.clock.now();
    station.tamper(|tx| forge_clinical_from(tx, 3, now));
    station.start().await;
    let verification = station.verify_targets().await;
    assert!(
        chain_intact(&verification, "clinical"),
        "the forgery is locally consistent"
    );
    assert!(verification.summary.mismatched > 0, "{verification:?}");
    assert!(has_finding(&verification, "anchor_mismatch", "clinical"));
}

/// Replaces clinical events from `from` on with forged ones, recomputing every hash
/// with the audit crate itself, and re-seals the stream.
fn forge_clinical_from(tx: &Transaction<'_>, from: i64, now: lis_core::UtcTimestamp) {
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM audit_events WHERE stream = 'clinical' AND seq >= ?1",
            [from],
            |row| row.get(0),
        )
        .unwrap();
    assert!(count > 0);
    tx.execute(
        "DELETE FROM audit_seals WHERE stream = 'clinical' AND last_seq >= ?1",
        [from],
    )
    .unwrap();
    tx.execute(
        "DELETE FROM audit_events WHERE stream = 'clinical' AND seq >= ?1",
        [from],
    )
    .unwrap();
    let forger = Principal::system("forger", []);
    let attribution = Attribution {
        principal: &forger,
        occurred_at: now,
        recorded_at: now,
        correlation_id: CorrelationId::generate(),
        authority: &[],
    };
    for _ in 0..count {
        let event =
            AuditEvent::new(Stream::Clinical, "test.change", Outcome::Succeeded).reason("forged");
        lis_audit::append(tx, &attribution, &event).unwrap();
    }
    lis_audit::seal(tx, now).unwrap();
}

// VAL-40: deleting the newest events leaves an intact but shorter chain.
#[tokio::test]
async fn a_deleted_tail_is_local_events_missing() {
    let (mut station, _) = anchored().await;
    station.stop().await;
    station.tamper(|tx| {
        tx.execute(
            "DELETE FROM audit_seals WHERE stream = 'clinical' AND last_seq > 3",
            [],
        )
        .unwrap();
        tx.execute(
            "DELETE FROM audit_events WHERE stream = 'clinical' AND seq > 3",
            [],
        )
        .unwrap();
    });
    station.start().await;
    let verification = station.verify_targets().await;
    assert!(chain_intact(&verification, "clinical"));
    assert!(
        has_finding(&verification, "local_events_missing", "clinical"),
        "{verification:?}"
    );
    assert!(verification.problems() > 0);
}

// VAL-40: an older copy of the database put back in place.
#[tokio::test]
async fn an_older_database_swapped_in_is_local_events_missing() {
    let (mut station, _) = anchored().await;
    station.stop().await;
    let older = station.folders.path().join("older.sqlite3");
    station
        .raw()
        .execute("VACUUM INTO ?1", [older.to_str().unwrap()])
        .unwrap();
    station.changes(4, "synthetic change after the copy").await;
    station.start().await;
    station.export_anchors().await;
    station.stop().await;
    station.offline = None;
    let database = station.dir.path().join(DATABASE_FILE);
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(PathBuf::from(format!("{}{suffix}", database.display())));
    }
    fs::copy(&older, &database).unwrap();
    station.start().await;
    let verification = station.verify_targets().await;
    assert!(chain_intact(&verification, "clinical"));
    assert!(
        has_finding(&verification, "local_events_missing", "clinical"),
        "{verification:?}"
    );
    assert!(verification.summary.local_events_missing > 0);
}

// VAL-40: restoring an older set loses the anchored interval after it; the restore records
// explain it once the restore itself is anchored, and the anchors written after it match.
#[tokio::test]
async fn an_older_set_restored_gives_a_lost_interval_and_later_anchors_match() {
    let station = restored_older_set().await;

    let before = station.verify_targets().await;
    assert!(before.summary.unverified_restore_boundary > 0, "{before:?}");
    assert_eq!(before.summary.lost_interval, 0);

    station.export_anchors().await;
    let after = station.verify_targets().await;
    assert!(after.summary.lost_interval > 0, "{after:?}");
    assert_eq!(after.problems(), 0, "{after:?}");
    let matched = after.summary.matched;

    station.export_anchors().await;
    let later = station.verify_targets().await;
    assert_eq!(later.problems(), 0, "{later:?}");
    assert!(later.summary.matched > matched, "{later:?}");
}

/// An anchored station whose newest set is restored into a new installation after more
/// anchored changes, started on the restored data with the restore not yet anchored.
async fn restored_older_set() -> Station {
    let (mut station, target) = anchored().await;
    station.stop().await;
    let clock: Arc<dyn Clock> = station.clock.clone();
    station
        .offline(move |db, dir| backup_now(db, dir, clock.as_ref()))
        .await;
    station.advance(60);
    station
        .changes(5, "synthetic change lost with the restore")
        .await;
    station.start().await;
    station.export_anchors().await;
    station.stop().await;
    station.advance(60 * 60);
    let restored = restore_into_new(&station, &target).await;
    station.move_to(restored);
    station.start().await;
    station
}

/// The clinical head the station's one restore recorded.
fn restored_clinical_head(station: &Station) -> i64 {
    station
        .raw()
        .query_row(
            "SELECT seq FROM ops_restore_heads WHERE stream = 'clinical'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

/// Deletes the clinical events above `seq` and the seals that end above it.
fn truncate_clinical(tx: &Transaction<'_>, seq: i64) {
    tx.execute(
        "DELETE FROM audit_seals WHERE stream = 'clinical' AND last_seq > ?1",
        [seq],
    )
    .unwrap();
    tx.execute(
        "DELETE FROM audit_events WHERE stream = 'clinical' AND seq > ?1",
        [seq],
    )
    .unwrap();
}

/// After tampering, the restore record explains nothing: every truncated anchor is a
/// finding and none is a lost interval.
fn assert_truncation_unexplained(verification: &AnchorVerification) {
    assert!(
        has_finding(verification, "local_events_missing", "clinical")
            || has_finding(verification, "anchor_mismatch", "clinical"),
        "{verification:?}"
    );
    assert_eq!(verification.summary.lost_interval, 0, "{verification:?}");
    assert!(verification.problems() > 0, "{verification:?}");
}

// H1, NFR-18: a restore record moved later after the fact must not explain a truncation of
// the anchored events written after the restore.
#[tokio::test]
async fn a_restore_record_moved_later_does_not_explain_a_truncation() {
    let mut station = restored_older_set().await;
    station.export_anchors().await;
    station.stop().await;
    station
        .changes(4, "synthetic change after the restore")
        .await;
    station.start().await;
    station.export_anchors().await;
    assert_eq!(station.verify_targets().await.problems(), 0);
    station.stop().await;
    let head = restored_clinical_head(&station);
    station.tamper(|tx| {
        truncate_clinical(tx, head);
        tx.execute(
            "UPDATE ops_restores SET applied_at = '2099-01-01T00:00:00.000000000Z'",
            [],
        )
        .unwrap();
    });
    station.start().await;
    assert_truncation_unexplained(&station.verify_targets().await);
}

// H1, NFR-18: a restored head lowered after the fact must not explain a truncation of
// anchored events the restored set held.
#[tokio::test]
async fn a_restored_head_lowered_does_not_explain_a_truncation() {
    let mut station = restored_older_set().await;
    station.export_anchors().await;
    assert_eq!(station.verify_targets().await.problems(), 0);
    station.stop().await;
    station.tamper(|tx| {
        truncate_clinical(tx, 3);
        tx.execute(
            "UPDATE ops_restore_heads SET seq = 3 WHERE stream = 'clinical'",
            [],
        )
        .unwrap();
    });
    station.start().await;
    assert_truncation_unexplained(&station.verify_targets().await);
}

/// Stages `target`'s newest set into a new installation, confirms, applies and records it,
/// as the engine will at its next start (the backup area's restore commands).
async fn restore_into_new(station: &Station, target: &Target) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let clock: Arc<dyn Clock> = station.clock.clone();
    let root = targets::root(target);
    let path = dir.path().to_owned();
    tokio::task::spawn_blocking(move || {
        let fresh = Database::open(&path, MODULES, clock.clone()).unwrap();
        write(&fresh, |ctx| lis_ops::ensure_installation(ctx).map(drop));
        drop(fresh);
        let key = BackupKey::derive(&installation_key());
        let staged = stage(&StageInput {
            source_root: &root,
            set_id: None,
            key: &key,
            scratch_dir: &restore_staging_dir(&path),
            modules: MODULES,
            artifact_columns: ARTIFACT_COLUMNS,
            anchor_roots: &[],
            keep_artifacts: true,
            clock: clock.clone(),
            cancel: &AtomicBool::new(false),
        })
        .unwrap();
        assert!(staged.passed(), "{:?}", staged.findings);
        let restore_id = confirm_restore(&path, &staged, clock.now()).unwrap();
        let applied = apply_staged(&path, clock.now(), |_| false).unwrap();
        assert_eq!(applied, ApplyOutcome::Applied(restore_id));
        let db = Database::open(&path, MODULES, clock).unwrap();
        let marker = applied_restore(&path).unwrap().unwrap();
        db.write_blocking(
            Principal::system(RESTORE_COMPONENT, [Capability::ManageQuality]),
            CorrelationId::generate(),
            move |ctx| {
                record_applied_restore(ctx, &marker, |_| {
                    Ok::<_, lis_ops::backup::BackupError>(RestoreFloors {
                        accession: "A26001001".to_owned(),
                        container: "C00005001".to_owned(),
                    })
                })
            },
        )
        .unwrap();
        drop(db);
        finish_recorded_restore(&path).unwrap();
    })
    .await
    .unwrap();
    dir
}

// NFR-18, ADR 0008: anchor files hold the ten digest-only keys and nothing typed by
// anyone: no reason, label or path.
#[tokio::test]
async fn anchor_files_hold_only_the_allowed_keys_and_no_canary() {
    const CANARY: &str = "SYNTHCANARYANCHOR";
    let mut station = Station::new().await;
    let target = station
        .add_target(
            "SYNTHCANARYANCHOR-folder",
            "SYNTHCANARYANCHOR label",
            TargetKind::NetworkShare,
        )
        .await;
    station.stop().await;
    station.changes(3, "SYNTHCANARYANCHOR reason").await;
    station.start().await;
    station.export_anchors().await;
    let installation: String = station
        .raw()
        .query_row("SELECT installation_id FROM ops_installation", [], |row| {
            row.get(0)
        })
        .unwrap();
    let allowed = [
        "format",
        "installation",
        "stream",
        "seal",
        "firstSeq",
        "lastSeq",
        "prevHash",
        "headHash",
        "sealedAt",
        "hashAlg",
    ];
    let mut lines = 0;
    for file in anchor_files(&targets::root(&target)) {
        let bytes = fs::read(&file).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains(CANARY), "an anchor file holds typed text");
        assert!(text.ends_with('\n'));
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            let object = value.as_object().unwrap();
            let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let mut expected = allowed.to_vec();
            expected.sort_unstable();
            assert_eq!(keys, expected);
            assert_eq!(object["format"], "vibe-tox-anchor-1");
            assert_eq!(object["hashAlg"], "sha256-v1");
            assert_eq!(object["installation"], installation.as_str());
            lines += 1;
        }
    }
    assert!(lines >= 2, "{lines}");
}

/// Every `*.jsonl` file under `root`.
fn anchor_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_owned()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path);
            }
        }
    }
    files
}

// NFR-18: an absent off-host target raises the lag; it drains when the target is back.
#[tokio::test]
async fn stopping_anchoring_raises_lag_until_the_target_returns() {
    let (station, target) = anchored().await;
    let status = station.engine().get_anchor_status().await.unwrap();
    assert_eq!(status.off_host_targets, 1);
    assert!(status.lag_minutes.unwrap_or(0) <= 1, "{status:?}");

    let away = station.folders.path().join("usb-unplugged");
    fs::rename(&target.path, &away).unwrap();
    station.advance(45 * 60);
    station.sign_in().await;
    let refused = station.engine().export_anchors_now().await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    let status = station.engine().get_anchor_status().await.unwrap();
    assert!(status.lag_minutes.unwrap() >= 45, "{status:?}");

    fs::rename(&away, &target.path).unwrap();
    let report = station.export_anchors().await;
    assert_eq!(report.targets_written, 1);
    let status = station.engine().get_anchor_status().await.unwrap();
    assert!(status.lag_minutes.unwrap_or(0) <= 1, "{status:?}");
}

// ADR 0008: anchor lag never blocks release, even past the 24-hour incident threshold.
#[tokio::test]
async fn release_continues_while_nothing_is_anchored() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Desk(Role::Admin)).await;
    let lab = bench.lab();
    lab.t().create_user("aud1", &[RoleCode::Auditor]).await;
    let (_, auditor) = lab.t().first_sign_in("aud1").await;
    let status = lab.engine().get_anchor_status().await.unwrap();
    assert_eq!(status.off_host_targets, 0);
    lab.t().advance(25 * 60 * 60);
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    bench.release(&candidate).await.unwrap();
    lab.t().sign_in_with("aud1", NEW_PASSWORD, &auditor).await;
    let status = lab.engine().get_anchor_status().await.unwrap();
    assert!(status.lag_minutes.unwrap() >= 24 * 60, "{status:?}");
}

// NFR-12, REQ-OPS-010: an exit export of synthetic data reads with the independent
// standard-library verifier (run when `VIBE_TOX_RUN_PY_VERIFIER=1`, as CI does on Linux).
#[tokio::test]
async fn an_exit_export_passes_the_independent_verifier() {
    let (mut station, _) = anchored().await;
    station.stop().await;
    let exports = station.folders.path().join("exports");
    fs::create_dir_all(&exports).unwrap();
    let folder = exports.clone();
    let package = station
        .offline(move |db, dir| {
            db.artifacts().put(b"synthetic stored file").unwrap();
            let dir = dir.to_owned();
            let request_dir = dir.clone();
            let request_folder = folder.clone();
            let export_id = write(db, move |ctx| {
                exit_export::request_exit_export(
                    ctx,
                    &request_dir,
                    &request_folder,
                    "Synthetic exit rehearsal",
                )
            });
            let report = exit_export::run_exit_export(&ExitExportJob {
                db,
                data_dir: &dir,
                export_id,
                folder: &folder,
                modules: MODULES,
                artifact_columns: ARTIFACT_COLUMNS,
                cancel: &AtomicBool::new(false),
            })
            .unwrap();
            assert_eq!(report.state, ExportState::Succeeded, "{report:?}");
            folder.join(report.package_name.unwrap())
        })
        .await;
    assert!(package.join("manifest.json").is_file());
    if std::env::var("VIBE_TOX_RUN_PY_VERIFIER").as_deref() != Ok("1") {
        return;
    }
    let (passed, report) = run_verifier(&package);
    assert!(passed, "{report}");
    assert!(report.contains("RESULT: VERIFIED"), "{report}");

    // A damaged stored file and a missing manifest are each reported and fail.
    let artifact = fs::read_dir(package.join("artifacts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::read(&artifact).unwrap();
    let mut damaged = original.clone();
    damaged[0] ^= 1;
    fs::write(&artifact, &damaged).unwrap();
    let (passed, report) = run_verifier(&package);
    assert!(!passed && report.contains("FAIL"), "{report}");
    fs::write(&artifact, &original).unwrap();
    fs::rename(package.join("manifest.json"), exports.join("manifest.json")).unwrap();
    let (passed, report) = run_verifier(&package);
    assert!(!passed && report.contains("manifest.json"), "{report}");
}

/// Runs `scripts/verify_exit_export.py` on `package` in isolated mode; returns whether it
/// exited 0, and its report.
fn run_verifier(package: &Path) -> (bool, String) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/verify_exit_export.py");
    let output = std::process::Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(package)
        .output()
        .expect("python3 runs");
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    (output.status.success(), report)
}

/// The anchors area's authorization-table calls (`authorization.rs`).
pub(crate) async fn anchors_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    Some(match name {
        "get_anchor_status" => engine.get_anchor_status().await.map(drop),
        "list_anchor_exports" => engine
            .list_anchor_exports(ListAnchorExportsInput { limit: 10 })
            .await
            .map(drop),
        "export_anchors_now" => engine.export_anchors_now().await.map(drop),
        "verify_audit_anchors" => engine.verify_audit_anchors(None).await.map(drop),
        _ => return None,
    })
}
