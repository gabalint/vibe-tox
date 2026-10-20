//! Consistent database snapshots with `VACUUM INTO` (ADR 0008 item 5), and
//! the copy kept before migrations change an existing database (NFR-14, M4
//! design brief §6.4). Backup targets, encryption and manifests are in
//! `lis-ops`.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use lis_core::UtcTimestamp;
use rusqlite::Connection;

use crate::artifacts::{ArtifactDigest, sync_dir};
use crate::database::BUSY_TIMEOUT;
use crate::error::StoreError;

/// A snapshot file with its integrity-check result. Callers must check
/// [`is_intact`](Self::is_intact) before trusting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Where the snapshot was written.
    pub path: PathBuf,
    /// SHA-256 of the snapshot file.
    pub digest: ArtifactDigest,
    /// Size of the snapshot file in bytes.
    pub size: u64,
    /// The rows `PRAGMA integrity_check` returned for the snapshot: `["ok"]` when intact.
    pub integrity_check: Vec<String>,
}

impl Snapshot {
    /// True when the integrity check found no problem.
    #[must_use]
    pub fn is_intact(&self) -> bool {
        self.integrity_check == ["ok"]
    }
}

/// Writes one committed state of `database_file` to `destination`, which must
/// not exist yet, then checks and digests the copy. Runs on its own
/// connection, so it neither blocks the writer nor uses a reader.
pub(crate) fn snapshot(database_file: &Path, destination: &Path) -> Result<Snapshot, StoreError> {
    let target = destination
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "snapshot path is not UTF-8"))?;
    let source = Connection::open(database_file)?;
    source.busy_timeout(BUSY_TIMEOUT)?;
    source.execute("VACUUM INTO ?1", [target])?;
    drop(source);
    let integrity_check = integrity_check(destination)?;
    OpenOptions::new()
        .write(true)
        .open(destination)?
        .sync_all()?;
    if let Some(parent) = destination.parent() {
        sync_dir(parent)?;
    }
    let (digest, size) = ArtifactDigest::of_reader(File::open(destination)?)?;
    Ok(Snapshot {
        path: destination.to_path_buf(),
        digest,
        size,
        integrity_check,
    })
}

/// Folder in the data directory that keeps the copies taken before migrations.
pub const PRE_MIGRATION_DIR: &str = "pre-migration";
/// How many pre-migration copies are kept, newest first.
const PRE_MIGRATION_KEPT: usize = 3;
const SNAPSHOT_SUFFIX: &str = ".sqlite3";
const TEMP_SUFFIX: &str = ".sqlite3.tmp";

/// Copies `database_file` to `pre-migration/<UTC timestamp>.sqlite3` in
/// `data_dir` before migrations run (NFR-14), keeping only the newest
/// [`PRE_MIGRATION_KEPT`] copies.
///
/// The copy is written under a temporary name and gets its final name only
/// after its integrity check passes, so every kept copy is intact. Any failure
/// is a `PreMigrationSnapshot*` error and the caller must not migrate.
pub(crate) fn pre_migration_snapshot(
    data_dir: &Path,
    database_file: &Path,
    now: UtcTimestamp,
) -> Result<(), StoreError> {
    let dir = data_dir.join(PRE_MIGRATION_DIR);
    let failed = |error: StoreError| StoreError::PreMigrationSnapshotFailed(Box::new(error));
    fs::create_dir_all(&dir)
        .and_then(|()| sync_dir(data_dir))
        .map_err(|error| failed(error.into()))?;
    // `20261013T101500.123456789Z`: lexical order is chronological, and the
    // name is valid on every supported file system.
    let stamp = now.to_string().replace(['-', ':'], "");
    let temp = dir.join(format!("{stamp}{TEMP_SUFFIX}"));
    let taken = snapshot(database_file, &temp).map_err(failed)?;
    if !taken.is_intact() {
        fs::remove_file(&temp).map_err(|error| failed(error.into()))?;
        return Err(StoreError::PreMigrationSnapshotNotIntact);
    }
    fs::rename(&temp, dir.join(format!("{stamp}{SNAPSHOT_SUFFIX}")))
        .and_then(|()| sync_dir(&dir))
        .and_then(|()| prune_pre_migration(&dir))
        .map_err(|error| failed(error.into()))?;
    tracing::info!(digest = %taken.digest, size = taken.size, "pre-migration snapshot taken");
    Ok(())
}

/// Removes temporary files left by an interrupted copy, and every copy but
/// the newest [`PRE_MIGRATION_KEPT`]. Other files are left alone.
fn prune_pre_migration(dir: &Path) -> io::Result<()> {
    let mut kept = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.ends_with(TEMP_SUFFIX) {
            fs::remove_file(&path)?;
        } else if name.ends_with(SNAPSHOT_SUFFIX) {
            kept.push(path);
        }
    }
    kept.sort();
    let excess = kept.len().saturating_sub(PRE_MIGRATION_KEPT);
    for path in &kept[..excess] {
        fs::remove_file(path)?;
    }
    sync_dir(dir)
}

fn integrity_check(path: &Path) -> Result<Vec<String>, StoreError> {
    let conn = Connection::open(path)?;
    let mut statement = conn.prepare("PRAGMA integrity_check")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
