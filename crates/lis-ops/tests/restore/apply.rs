//! Restore onto a new installation (REQ-OPS-007, REQ-OPS-002; M4 design brief D4, §3.7):
//! the session, the source listing, staging, the markers, applying before the database
//! opens (resumable, never deleting the replaced database) and recording after it.

use std::fs;
use std::path::Path;
use std::time::Duration;

use lis_core::ErrorCode;
use lis_ops::backup::BackupError;
use lis_ops::kit::key_text;
use lis_ops::recovery;
use lis_ops::restore::{
    ApplyFailure, ApplyOutcome, FindingCode, MarkerState, RestoreSession, StagedRestore,
    apply_staged, cancel_restore, clean_scratch, confirm_restore, pending_restore, read_source,
    restore_staging_dir,
};
use lis_ops::targets::{self, TargetKind};
use lis_store::{ArtifactDigest, DATABASE_FILE, Database, StoreError};

use crate::support::{
    DEMO, Fixture, MODULES, backed_up, installation_key, new_installation, record, restore_into_new,
};

fn digest(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    Some(ArtifactDigest::of_reader(file).unwrap().0.to_string())
}

/// Every file under `dir`, relative, sorted.
fn files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap();
                found.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_restore_is_applied_before_the_open_and_recorded_after_it() {
    let (f, target, set_id) = backed_up();
    let new = new_installation(&f);
    let data_dir = new.path();
    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    assert!(staged.passed(), "{:?}", staged.findings);
    assert_eq!(staged.set_id, set_id);
    assert!(
        staged
            .chains
            .iter()
            .all(|chain| chain.intact && chain.head_matches_manifest)
    );
    let restore_id = confirm_restore(data_dir, &staged, f.now()).unwrap();
    assert_eq!(
        pending_restore(data_dir).unwrap().unwrap().state,
        MarkerState::Ready
    );

    let applied = apply_staged(data_dir, f.now(), |_| false).unwrap();
    assert_eq!(applied, ApplyOutcome::Applied(restore_id));
    let replaced = check_applied_files(data_dir, &staged);
    // Applying again changes nothing until the restore is recorded.
    assert_eq!(
        apply_staged(data_dir, f.now(), |_| false).unwrap(),
        ApplyOutcome::Applied(restore_id)
    );
    assert_eq!(files(&data_dir.join("ops").join("replaced")), replaced);

    let db = Database::open(data_dir, &MODULES, f.clock()).unwrap();
    assert_eq!(record(&db, data_dir), restore_id);
    // A crash between the recording and the marker's removal records nothing twice.
    assert_eq!(record(&db, data_dir), restore_id);
    lis_ops::restore::finish_recorded_restore(data_dir).unwrap();
    assert!(!restore_staging_dir(data_dir).exists());
    assert!(data_dir.join("ops").join("replaced").exists());
    check_recorded(&f, &db);
}

/// The restored database and artifacts are in place and the replaced database is kept;
/// returns the replaced files.
fn check_applied_files(data_dir: &Path, staged: &StagedRestore) -> Vec<String> {
    assert_eq!(digest(&data_dir.join(DATABASE_FILE)), staged.staged_sha256);
    let replaced = files(&data_dir.join("ops").join("replaced"));
    let database: Vec<&String> = replaced
        .iter()
        .filter(|name| name.ends_with(DATABASE_FILE))
        .collect();
    assert_eq!(database.len(), 1, "{replaced:?}");
    let kept = data_dir.join("ops").join("replaced").join(database[0]);
    let marker = pending_restore(data_dir).unwrap().unwrap();
    assert_eq!(marker.state, MarkerState::Applied);
    assert_eq!(
        digest(&kept),
        marker.marker.replaced_database_sha256,
        "kept, with its journal"
    );
    for artifact in &staged.manifest.as_ref().unwrap().artifacts {
        let path = data_dir.join("artifacts").join(&artifact.sha256);
        assert_eq!(digest(&path).as_deref(), Some(artifact.sha256.as_str()));
    }
    replaced
}

/// One restore, its three heads and its event are recorded, and the gate is open.
fn check_recorded(f: &Fixture, db: &Database) {
    let (restores, heads, events, installation) = db
        .read_blocking(|conn| -> Result<_, StoreError> {
            let count = |sql: &str| conn.query_row(sql, [], |row| row.get::<_, i64>(0));
            Ok((
                count("SELECT count(*) FROM ops_restores")?,
                count("SELECT count(*) FROM ops_restore_heads")?,
                count(
                    "SELECT count(*) FROM audit_events WHERE action = 'ops.restore_applied' \
                     AND principal_id = 'restore'",
                )?,
                lis_ops::installation_id(conn)?,
            ))
        })
        .unwrap();
    assert_eq!((restores, heads, events), (1, 3, 1));
    let original = f.test.db.read_blocking(lis_ops::installation_id).unwrap();
    assert_eq!(
        installation, original,
        "the restored installation keeps its ID"
    );
    assert!(db.read_blocking(recovery::is_open).unwrap());
}

/// Rewrites the marker of `from` as `to`, with the time applying began, as a crash
/// between two steps leaves it.
fn crash_marker(data_dir: &Path, from: &str, to: &str, applied_at: &str) {
    let staging = restore_staging_dir(data_dir);
    let mut marker: serde_json::Value =
        serde_json::from_slice(&fs::read(staging.join(from)).unwrap()).unwrap();
    marker["appliedAt"] = serde_json::Value::String(applied_at.to_owned());
    fs::write(staging.join(to), serde_json::to_vec(&marker).unwrap()).unwrap();
    fs::remove_file(staging.join(from)).unwrap();
}

#[test]
fn applying_resumes_after_a_crash_and_never_moves_the_restored_database_away() {
    let (f, target, _) = backed_up();
    let new = new_installation(&f);
    let data_dir = new.path();
    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    let restore_id = confirm_restore(data_dir, &staged, f.now()).unwrap();
    let now = f.now().to_string();
    crash_marker(data_dir, "READY.json", "APPLYING.json", &now);
    assert_eq!(
        pending_restore(data_dir).unwrap().unwrap().state,
        MarkerState::Applying
    );

    let resumed = apply_staged(data_dir, f.now(), |_| panic!("not rechecked once applying"));
    assert_eq!(resumed.unwrap(), ApplyOutcome::Applied(restore_id));
    assert_eq!(digest(&data_dir.join(DATABASE_FILE)), staged.staged_sha256);
    let replaced = data_dir.join("ops").join("replaced");
    let kept = files(&replaced);
    assert!(
        kept.iter().any(|name| name.ends_with(DATABASE_FILE)),
        "{kept:?}"
    );

    // A crash after every file moved but before the marker: resuming moves nothing.
    crash_marker(data_dir, "APPLIED.json", "APPLYING.json", &now);
    let again = apply_staged(data_dir, f.now(), |_| false).unwrap();
    assert_eq!(again, ApplyOutcome::Applied(restore_id));
    assert_eq!(digest(&data_dir.join(DATABASE_FILE)), staged.staged_sha256);
    assert_eq!(files(&replaced), kept);
}

#[test]
fn applying_is_refused_when_users_exist_or_the_staged_database_changed() {
    let (f, target, _) = backed_up();
    let new = new_installation(&f);
    let data_dir = new.path();
    let current = digest(&data_dir.join(DATABASE_FILE));
    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    confirm_restore(data_dir, &staged, f.now()).unwrap();
    let refused = apply_staged(data_dir, f.now(), |_| true).unwrap();
    assert_eq!(refused, ApplyOutcome::Failed(ApplyFailure::UsersExist));
    let pending = pending_restore(data_dir).unwrap().unwrap();
    assert_eq!(pending.state, MarkerState::Failed);
    assert_eq!(pending.marker.failure, Some(ApplyFailure::UsersExist));
    assert_eq!(digest(&data_dir.join(DATABASE_FILE)), current, "untouched");
    assert!(!data_dir.join("ops").join("replaced").exists());
    assert!(cancel_restore(data_dir).unwrap());
    assert_eq!(
        apply_staged(data_dir, f.now(), |_| false).unwrap(),
        ApplyOutcome::Nothing
    );

    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    confirm_restore(data_dir, &staged, f.now()).unwrap();
    let file = restore_staging_dir(data_dir).join(DATABASE_FILE);
    let mut bytes = fs::read(&file).unwrap();
    bytes.extend_from_slice(b"\0");
    fs::write(&file, bytes).unwrap();
    let changed = apply_staged(data_dir, f.now(), |_| false).unwrap();
    assert_eq!(
        changed,
        ApplyOutcome::Failed(ApplyFailure::StagedDatabaseChanged)
    );
    assert_eq!(digest(&data_dir.join(DATABASE_FILE)), current);
}

#[test]
fn changed_artifacts_are_refused_before_apply_and_on_applying_resume() {
    let (f, target, _) = backed_up();
    let new = new_installation(&f);
    let data_dir = new.path();
    let current = digest(&data_dir.join(DATABASE_FILE));
    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    confirm_restore(data_dir, &staged, f.now()).unwrap();
    let artifact = staged.manifest.as_ref().unwrap().artifacts[0]
        .sha256
        .clone();
    let staged_file = restore_staging_dir(data_dir)
        .join("artifacts")
        .join(&artifact);
    fs::write(&staged_file, b"changed after staging").unwrap();
    assert_eq!(
        apply_staged(data_dir, f.now(), |_| false).unwrap(),
        ApplyOutcome::Failed(ApplyFailure::StagedArtifactsChanged)
    );
    assert_eq!(
        digest(&data_dir.join(DATABASE_FILE)),
        current,
        "database untouched"
    );

    cancel_restore(data_dir).unwrap();
    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    confirm_restore(data_dir, &staged, f.now()).unwrap();
    let now = f.now().to_string();
    crash_marker(data_dir, "READY.json", "APPLYING.json", &now);
    let artifact = &staged.manifest.as_ref().unwrap().artifacts[0].sha256;
    let staged_file = restore_staging_dir(data_dir)
        .join("artifacts")
        .join(artifact);
    let live_file = data_dir.join("artifacts").join(artifact);
    fs::create_dir_all(live_file.parent().unwrap()).unwrap();
    fs::rename(staged_file, &live_file).unwrap();
    fs::write(&live_file, b"changed reused live artifact").unwrap();
    assert_eq!(
        apply_staged(data_dir, f.now(), |_| panic!(
            "users are not rechecked on resume"
        ))
        .unwrap(),
        ApplyOutcome::Failed(ApplyFailure::StagedArtifactsChanged)
    );
    assert_eq!(
        digest(&data_dir.join(DATABASE_FILE)),
        current,
        "resume did not swap"
    );
}

#[test]
fn a_replaced_directory_collision_is_refused_and_includes_the_restore_id() {
    let (f, target, _) = backed_up();
    let new = new_installation(&f);
    let data_dir = new.path();
    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    let restore_id = confirm_restore(data_dir, &staged, f.now()).unwrap();
    let stamp = f.now().to_string().replace(['-', ':'], "");
    let collision = data_dir
        .join("ops")
        .join("replaced")
        .join(format!("{stamp}-{restore_id}"));
    fs::create_dir_all(&collision).unwrap();

    assert_eq!(
        apply_staged(data_dir, f.now(), |_| false).unwrap(),
        ApplyOutcome::Failed(ApplyFailure::ReplacedDirectoryExists)
    );
    assert!(collision.exists());
    assert!(restore_staging_dir(data_dir).join(DATABASE_FILE).exists());
}

#[test]
fn only_a_staged_set_that_passed_is_confirmed() {
    let (f, target, _) = backed_up();
    let artifacts = targets::root(&target).join("artifacts");
    let first = fs::read_dir(&artifacts)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::remove_file(first).unwrap();
    let new = new_installation(&f);
    let staged = f.stage_into(&target, &restore_staging_dir(new.path()));
    assert!(staged.findings.count(FindingCode::ArtifactMissing) == 1);
    let refused = confirm_restore(new.path(), &staged, f.now()).unwrap_err();
    assert!(matches!(refused, BackupError::RestoreNotVerified));
    assert!(pending_restore(new.path()).unwrap().is_none());
}

#[test]
fn the_session_keeps_the_key_thirty_minutes() {
    let f = Fixture::new();
    let session = RestoreSession::begin(&key_text(&installation_key()), f.now()).unwrap();
    assert_eq!(session.key_id(), f.key.key_id());
    let later = |minutes: u64| {
        f.now()
            .checked_add(Duration::from_secs(minutes * 60))
            .unwrap()
    };
    assert!(session.backup_key(later(29)).is_ok());
    let expired = session.installation_key(later(30)).unwrap_err();
    assert!(matches!(expired, BackupError::RestoreSessionExpired));
    assert!(!format!("{session:?}").contains(&*key_text(&installation_key())));
    let typo = RestoreSession::begin("vtk1-0000", f.now()).unwrap_err();
    assert_eq!(typo.code(), ErrorCode::Validation);
}

#[test]
fn a_chosen_folder_lists_its_sets_with_their_compatibility() {
    let (f, target, set_id) = backed_up();
    let source = read_source(&target.path, &f.key, &MODULES).unwrap();
    assert_eq!(source.sets.len(), 1);
    assert_eq!(source.sets[0].set_id, set_id);
    assert_eq!(source.sets[0].incompatible, None);
    assert_eq!(source.root, targets::root(&target));
    let older_build = read_source(&target.path, &f.key, &[lis_ops::SCHEMA, DEMO]).unwrap();
    assert_eq!(
        older_build.sets[0].incompatible,
        Some(FindingCode::SchemaNewerThanApp)
    );

    let other = lis_ops::format::BackupKey::derive(&lis_auth::DataKey::from_bytes([9; 32]));
    let wrong = read_source(&target.path, &other, &MODULES).unwrap_err();
    assert!(matches!(wrong, BackupError::TargetKeyMismatch));
    let empty = read_source(f.folders.path(), &f.key, &MODULES).unwrap_err();
    assert!(matches!(empty, BackupError::NoBackupsInFolder));
}

#[test]
fn startup_cleanup_keeps_a_confirmed_restore_and_removes_scratch() {
    let (f, target, _) = backed_up();
    let restored = restore_into_new(&f, &target);
    let data_dir = restored.dir.path();
    let tests = data_dir.join("ops").join("restore-tests").join("leftover");
    fs::create_dir_all(&tests).unwrap();
    fs::create_dir_all(restore_staging_dir(data_dir)).unwrap();
    clean_scratch(data_dir).unwrap();
    assert!(!tests.exists());
    assert!(!restore_staging_dir(data_dir).exists());
    assert!(data_dir.join("ops").join("replaced").exists());

    let staged = f.stage_into(&target, &restore_staging_dir(data_dir));
    confirm_restore(data_dir, &staged, f.now()).unwrap();
    clean_scratch(data_dir).unwrap();
    assert_eq!(
        pending_restore(data_dir).unwrap().unwrap().state,
        MarkerState::Ready
    );
}

#[test]
fn a_restore_test_needs_a_known_target_the_capability_and_no_waiting_test() {
    let f = Fixture::new();
    let target = f.target("usb", TargetKind::Removable);
    let unknown = lis_ops::targets::TargetId::generate();
    let error = f
        .test
        .db
        .write_blocking(
            crate::support::admin(),
            lis_core::CorrelationId::generate(),
            move |ctx| lis_ops::restore::request_restore_test(ctx, unknown, None),
        )
        .unwrap_err();
    assert!(matches!(error, BackupError::NotFound(_)));
    let target_id = target.id;
    let request = move |ctx: &mut lis_store::CommandContext<'_>| {
        lis_ops::restore::request_restore_test(ctx, target_id, None)
    };
    f.write(crate::support::admin(), request);
    let twice = f
        .test
        .db
        .write_blocking(
            crate::support::admin(),
            lis_core::CorrelationId::generate(),
            request,
        )
        .unwrap_err();
    assert!(matches!(twice, BackupError::RestoreTestQueued));
    let forbidden = f
        .test
        .db
        .write_blocking(
            lis_store::testing::principal(&[lis_core::Capability::Export]),
            lis_core::CorrelationId::generate(),
            request,
        )
        .unwrap_err();
    assert_eq!(forbidden.code(), ErrorCode::Forbidden);
}
