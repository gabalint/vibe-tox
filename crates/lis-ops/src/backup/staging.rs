//! The plaintext staging copy of a run (M4 design brief §3.1 steps 3–4, ground rules):
//! the snapshot waits inside the data folder, under `ops/staging/`, and is removed when
//! the run ends; snapshots an earlier run left behind are removed before the next one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lis_store::DATABASE_FILE;

/// Plaintext snapshots wait here, inside the data folder, while a run copies them.
const STAGING_DIR: [&str; 2] = ["ops", "staging"];
/// Name suffix of a staged snapshot.
pub(super) const SNAPSHOT_SUFFIX: &str = ".sqlite3";
/// Free space beyond 1.2 × the database that a snapshot needs.
const SPACE_MARGIN_BYTES: u64 = 1 << 30;

/// The staging folder of `data_dir`.
pub(super) fn staging_dir(data_dir: &Path) -> PathBuf {
    STAGING_DIR
        .iter()
        .fold(data_dir.to_path_buf(), |path, part| path.join(part))
}

/// Removes every plaintext backup snapshot left by a stopped process. Called
/// at startup so the copy does not wait for another backup run (REQ-OPS-007).
pub fn clean_staging(data_dir: &Path) -> io::Result<()> {
    let staging = staging_dir(data_dir);
    match fs::remove_dir_all(staging) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Free space on the data folder's disk exceeds 1.2 × the database (with its WAL) + 1 GiB.
pub(super) fn enough_space(data_dir: &Path) -> io::Result<bool> {
    let mut database = fs::metadata(data_dir.join(DATABASE_FILE))?.len();
    match fs::metadata(data_dir.join(format!("{DATABASE_FILE}-wal"))) {
        Ok(wal) => database += wal.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let needed = database + database / 5 + SPACE_MARGIN_BYTES;
    Ok(fs4::available_space(data_dir)? > needed)
}

/// Creates the staging folder and removes snapshots an interrupted run left there.
pub(super) fn prepare_staging(staging: &Path) -> io::Result<()> {
    fs::create_dir_all(staging)?;
    clear_staging(staging)
}

/// Removes every snapshot file (with its journal files) from the staging folder.
pub(super) fn clear_staging(staging: &Path) -> io::Result<()> {
    for entry in fs::read_dir(staging)? {
        let path: PathBuf = entry?.path();
        let is_snapshot = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains(SNAPSHOT_SUFFIX));
        if is_snapshot {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_cleanup_removes_plaintext_staging_and_keeps_siblings() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let staging = staging_dir(root.path());
        fs::create_dir_all(&staging)?;
        fs::write(staging.join("synthetic.sqlite3"), b"synthetic snapshot")?;
        let sibling = root.path().join("ops").join("keep.txt");
        fs::write(&sibling, b"keep")?;

        clean_staging(root.path())?;

        assert!(!staging.exists());
        assert_eq!(fs::read(sibling)?, b"keep");
        clean_staging(root.path())?;
        Ok(())
    }
}
