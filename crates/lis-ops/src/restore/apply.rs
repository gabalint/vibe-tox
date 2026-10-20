//! Restore onto a new installation, in files (M4 design brief D4, §3.7): a verified set
//! is staged in `ops/restore-staging/`, confirmed with a `READY.json` marker, and applied
//! at the next start before the database opens.
//!
//! Marker states, one file each, most advanced first: `APPLIED.json` (in place, waiting
//! to be recorded in the restored database), `APPLYING.json` (files being moved;
//! resumable), `FAILED.json` (a recheck failed; nothing was moved), `READY.json`.
//!
//! Invariants: a database or artifact being replaced is moved to
//! `ops/replaced/<timestamp>/`, never deleted; every step of applying is idempotent, so a
//! crash while `APPLYING` resumes at the next start; the staged database's digest and the
//! absence of user accounts are rechecked before anything moves.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use lis_core::{Id, UtcTimestamp};
use lis_store::{ARTIFACTS_DIR, ArtifactDigest, DATABASE_FILE};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use super::anchors::{ANCHOR_COUNTS, counts};
use super::artifacts::{move_artifacts, recheck_artifacts};
use super::stage::StagedRestore;
use crate::backup::durable::{rename, write_file};
use crate::backup::{ArtifactEntry, BackupError, HeadEntry, RunKind};

/// Where a confirmed restore waits, inside the data folder.
const STAGING_DIR: [&str; 2] = ["ops", "restore-staging"];
/// Where replaced databases and artifacts are kept, inside the data folder.
const REPLACED_DIR: [&str; 2] = ["ops", "replaced"];
const MARKER_FORMAT: &str = "vibe-tox-restore-1";
/// The database file and its journal files.
const DATABASE_SUFFIXES: [&str; 3] = ["", "-wal", "-shm"];

/// Marker type of restore IDs.
pub enum Restore {}
/// Identifies a restore; chosen when it is confirmed.
pub type RestoreId = Id<Restore>;

lis_core::text_enum! {
    /// Where a confirmed restore stands (M4 design brief §2.3).
    MarkerState {
        /// Staged, verified and confirmed.
        Ready = "ready",
        /// Files being moved into place; resumable.
        Applying = "applying",
        /// In place; recorded at the next database open.
        Applied = "applied",
        /// A recheck failed before anything moved.
        Failed = "failed",
    }
}

lis_core::text_enum! {
    /// Why applying a restore was refused.
    ApplyFailure {
        /// The staged database is missing or changed since it was verified.
        StagedDatabaseChanged = "staged_database_changed",
        /// The installation has user accounts now.
        UsersExist = "users_exist",
        /// A staged artifact or an artifact that would be reused changed after staging.
        StagedArtifactsChanged = "staged_artifacts_changed",
        /// This restore's unique replaced-files directory already existed before apply.
        ReplacedDirectoryExists = "replaced_directory_exists",
    }
}

impl MarkerState {
    /// Most advanced first, the order a reader trusts them in.
    const PRECEDENCE: [Self; 4] = [Self::Applied, Self::Applying, Self::Failed, Self::Ready];

    fn file_name(self) -> &'static str {
        match self {
            Self::Ready => "READY.json",
            Self::Applying => "APPLYING.json",
            Self::Applied => "APPLIED.json",
            Self::Failed => "FAILED.json",
        }
    }
}

/// The marker file's content: what is restored, and what applying did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RestoreMarker {
    format: String,
    /// The restore.
    pub restore_id: RestoreId,
    /// The set restored.
    pub set_id: String,
    /// When the set was taken.
    pub set_created_at: UtcTimestamp,
    /// Why its run started.
    pub set_kind: RunKind,
    /// SHA-256 of the set's database snapshot, from its manifest.
    pub snapshot_sha256: String,
    /// SHA-256 of the staged database file after its checks.
    pub staged_sha256: String,
    /// The app version that wrote the set.
    pub backup_app_version: String,
    /// The installation key's ID.
    pub key_id: String,
    /// When the set was confirmed.
    pub staged_at: UtcTimestamp,
    /// The set's chain heads.
    pub heads: Vec<HeadEntry>,
    /// Artifact digests and lengths copied from the authenticated set manifest. Apply
    /// rechecks these before any database move, including when resuming `APPLYING`.
    pub artifacts: Vec<ArtifactEntry>,
    /// The anchors compared while staging, in the stored count order.
    pub anchor_counts: [u64; ANCHOR_COUNTS],
    /// When applying began; it names the `ops/replaced/` folder.
    pub applied_at: Option<UtcTimestamp>,
    /// SHA-256 of the database file it replaced, if there was one.
    pub replaced_database_sha256: Option<String>,
    /// Why applying was refused.
    pub failure: Option<ApplyFailure>,
}

/// A marker found in the staging folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRestore {
    /// Its state.
    pub state: MarkerState,
    /// Its content.
    pub marker: RestoreMarker,
}

/// What [`apply_staged`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// No restore is waiting.
    Nothing,
    /// The restore is in place and waits to be recorded.
    Applied(RestoreId),
    /// The restore was refused; the current database is untouched.
    Failed(ApplyFailure),
}

/// The folder a restore is staged in; pass it to `stage` as the scratch folder.
#[must_use]
pub fn restore_staging_dir(data_dir: &Path) -> PathBuf {
    folder(data_dir, STAGING_DIR)
}

fn folder(data_dir: &Path, parts: [&str; 2]) -> PathBuf {
    parts
        .iter()
        .fold(data_dir.to_path_buf(), |path, part| path.join(part))
}

/// Confirms a staged restore that passed (`confirm_restore`): rechecks the staged file's
/// digest and writes `READY.json`. The engine rechecks that no user exists and writes the
/// installation key to the OS key store first, then quits.
pub fn confirm_restore(
    data_dir: &Path,
    staged: &StagedRestore,
    now: UtcTimestamp,
) -> Result<RestoreId, BackupError> {
    let (Some(manifest), Some(staged_sha256), true) =
        (&staged.manifest, &staged.staged_sha256, staged.passed())
    else {
        return Err(BackupError::RestoreNotVerified);
    };
    let dir = restore_staging_dir(data_dir);
    if file_digest(&dir.join(DATABASE_FILE))?.as_deref() != Some(staged_sha256.as_str()) {
        return Err(BackupError::RestoreNotVerified);
    }
    let marker = RestoreMarker {
        format: MARKER_FORMAT.to_owned(),
        restore_id: RestoreId::generate(),
        set_id: staged.set_id.clone(),
        set_created_at: manifest.created_at,
        set_kind: manifest.kind,
        snapshot_sha256: manifest.database.sha256.clone(),
        staged_sha256: staged_sha256.clone(),
        backup_app_version: manifest.app_version.clone(),
        key_id: manifest.key_id.clone(),
        staged_at: now,
        heads: manifest.heads.clone(),
        artifacts: manifest.artifacts.clone(),
        anchor_counts: staged.anchors.as_ref().map(counts).unwrap_or_default(),
        applied_at: None,
        replaced_database_sha256: None,
        failure: None,
    };
    write_marker(&dir, MarkerState::Ready, &marker)?;
    Ok(marker.restore_id)
}

/// The most advanced marker in the staging folder, if any.
pub fn pending_restore(data_dir: &Path) -> Result<Option<PendingRestore>, BackupError> {
    let dir = restore_staging_dir(data_dir);
    for state in MarkerState::PRECEDENCE {
        let bytes = match fs::read(dir.join(state.file_name())) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let marker: RestoreMarker =
            serde_json::from_slice(&bytes).map_err(|_| crate::format::FormatError::Corrupt)?;
        if marker.format != MARKER_FORMAT {
            return Err(crate::format::FormatError::Corrupt.into());
        }
        return Ok(Some(PendingRestore { state, marker }));
    }
    Ok(None)
}

/// Removes a staged restore that is not being applied (`cancel_restore`). Returns false,
/// and leaves everything, while a restore is applying or applied.
pub fn cancel_restore(data_dir: &Path) -> Result<bool, BackupError> {
    let state = pending_restore(data_dir)?.map(|pending| pending.state);
    if matches!(state, Some(MarkerState::Applying | MarkerState::Applied)) {
        return Ok(false);
    }
    remove_folder(&restore_staging_dir(data_dir))?;
    Ok(true)
}

/// Startup cleanup: removes restore-test scratch folders and a staging folder that holds
/// no marker. `ops/replaced/` is always kept.
pub fn clean_scratch(data_dir: &Path) -> Result<(), BackupError> {
    remove_folder(&folder(data_dir, super::test_runs::TESTS_DIR))?;
    if pending_restore(data_dir)?.is_none() {
        remove_folder(&restore_staging_dir(data_dir))?;
    }
    Ok(())
}

/// Applies a confirmed restore, before the database opens (§3.7). `users_exist` reads the
/// current database and returns true when user accounts exist or it cannot tell.
/// Idempotent: an `APPLYING` restore resumes; an applied one is reported again until it is
/// recorded. An I/O error leaves the marker `APPLYING`, so the next start resumes.
pub fn apply_staged(
    data_dir: &Path,
    now: UtcTimestamp,
    users_exist: impl FnOnce(&Connection) -> bool,
) -> Result<ApplyOutcome, BackupError> {
    let dir = restore_staging_dir(data_dir);
    let Some(PendingRestore { state, mut marker }) = pending_restore(data_dir)? else {
        return Ok(ApplyOutcome::Nothing);
    };
    let applied_at = marker.applied_at.unwrap_or(now);
    let replaced = replaced_path(data_dir, applied_at, marker.restore_id);
    match state {
        MarkerState::Applied => return Ok(ApplyOutcome::Applied(marker.restore_id)),
        MarkerState::Failed => {
            return Ok(marker
                .failure
                .map_or(ApplyOutcome::Nothing, ApplyOutcome::Failed));
        }
        MarkerState::Applying => {
            if let Some(failure) = recheck_files(data_dir, &marker)? {
                tracing::warn!(failure = failure.as_str(), "staged restore resume refused");
                return Ok(ApplyOutcome::Failed(failure));
            }
        }
        MarkerState::Ready => {
            if let Some(failure) = recheck(data_dir, &marker, &replaced, users_exist)? {
                marker.failure = Some(failure);
                move_marker(&dir, MarkerState::Ready, MarkerState::Failed, &marker)?;
                tracing::warn!(failure = failure.as_str(), "staged restore refused");
                return Ok(ApplyOutcome::Failed(failure));
            }
            marker.applied_at = Some(now);
            marker.replaced_database_sha256 = file_digest(&data_dir.join(DATABASE_FILE))?;
            move_marker(&dir, MarkerState::Ready, MarkerState::Applying, &marker)?;
        }
    }
    swap_database(data_dir, &dir, &replaced)?;
    move_artifacts(
        &dir.join(ARTIFACTS_DIR),
        &data_dir.join(ARTIFACTS_DIR),
        &replaced,
    )?;
    move_marker(&dir, MarkerState::Applying, MarkerState::Applied, &marker)?;
    tracing::info!(restore_id = %marker.restore_id, "staged restore applied");
    Ok(ApplyOutcome::Applied(marker.restore_id))
}

/// The staged database is the one verified, and the current database has no users.
fn recheck(
    data_dir: &Path,
    marker: &RestoreMarker,
    replaced: &Path,
    users_exist: impl FnOnce(&Connection) -> bool,
) -> Result<Option<ApplyFailure>, BackupError> {
    if replaced.exists() {
        return Ok(Some(ApplyFailure::ReplacedDirectoryExists));
    }
    if let Some(failure) = recheck_files(data_dir, marker)? {
        return Ok(Some(failure));
    }
    let current = data_dir.join(DATABASE_FILE);
    if current.exists() {
        let conn = Connection::open_with_flags(&current, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        if users_exist(&conn) {
            return Ok(Some(ApplyFailure::UsersExist));
        }
    }
    Ok(None)
}

/// Rechecks all authenticated staged state before any database move. On an `APPLYING`
/// resume the database or some artifacts may already be live, so each expected file is
/// accepted only in the one remaining location and is verified there again.
fn recheck_files(
    data_dir: &Path,
    marker: &RestoreMarker,
) -> Result<Option<ApplyFailure>, BackupError> {
    let staging = restore_staging_dir(data_dir);
    let staged_database = staging.join(DATABASE_FILE);
    let database = if staged_database.exists() {
        staged_database
    } else {
        data_dir.join(DATABASE_FILE)
    };
    if file_digest(&database)?.as_deref() != Some(marker.staged_sha256.as_str()) {
        return Ok(Some(ApplyFailure::StagedDatabaseChanged));
    }
    recheck_artifacts(data_dir, &staging, &marker.artifacts)
}

/// Moves the current database files to `replaced`, then the staged database into place.
/// Skipped once the staged database has moved, so a resumed apply never moves the
/// restored database away.
fn swap_database(data_dir: &Path, staging: &Path, replaced: &Path) -> io::Result<()> {
    let staged = staging.join(DATABASE_FILE);
    if !staged.exists() {
        return Ok(());
    }
    fs::create_dir_all(replaced)?;
    for suffix in DATABASE_SUFFIXES {
        let name = format!("{DATABASE_FILE}{suffix}");
        let current = data_dir.join(&name);
        if current.exists() {
            rename(&current, &replaced.join(&name))?;
        }
    }
    rename(&staged, &data_dir.join(DATABASE_FILE))
}

/// SHA-256 of a file, `None` when it does not exist.
pub(super) fn file_digest(path: &Path) -> io::Result<Option<String>> {
    match File::open(path) {
        Ok(file) => Ok(Some(ArtifactDigest::of_reader(file)?.0.to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// `20261013T101500.123456789Z`: sorts by time and is valid on every file system.
fn stamp(at: UtcTimestamp) -> String {
    at.to_string().replace(['-', ':'], "")
}

fn replaced_path(data_dir: &Path, at: UtcTimestamp, restore_id: RestoreId) -> PathBuf {
    folder(data_dir, REPLACED_DIR).join(format!("{}-{restore_id}", stamp(at)))
}

fn write_marker(dir: &Path, state: MarkerState, marker: &RestoreMarker) -> Result<(), BackupError> {
    let mut bytes = serde_json::to_vec(marker).map_err(|_| crate::format::FormatError::Corrupt)?;
    bytes.push(b'\n');
    write_file(&dir.join(state.file_name()), &bytes)?;
    Ok(())
}

/// Writes the marker of `to`, then removes the one of `from`; between the two, a reader
/// trusts the more advanced one.
fn move_marker(
    dir: &Path,
    from: MarkerState,
    to: MarkerState,
    marker: &RestoreMarker,
) -> Result<(), BackupError> {
    write_marker(dir, to, marker)?;
    fs::remove_file(dir.join(from.file_name()))?;
    lis_store::sync_dir(dir)?;
    Ok(())
}

/// Removes `dir` and everything in it; an absent folder is fine.
pub(super) fn remove_folder(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Removes the staging folder once its restore is recorded in the restored database.
pub(super) fn remove_staging(data_dir: &Path) -> io::Result<()> {
    remove_folder(&restore_staging_dir(data_dir))
}
