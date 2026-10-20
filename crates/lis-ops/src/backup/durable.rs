//! Durable file writes on target folders and in the data folder: write under an
//! unpredictable same-directory name, sync, rename, then sync the folder. Temporary
//! files are created exclusively and neither temporary nor final paths may be symlinks.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use rand::TryRng;
use rand::rngs::SysRng;

/// Suffix of a file or set folder that is still being written; never listed or read.
pub(crate) const TEMP_SUFFIX: &str = ".tmp";

/// An unpredictable same-directory temporary path derived from `path`.
pub(crate) fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let mut random = [0_u8; 16];
    SysRng
        .try_fill_bytes(&mut random)
        .map_err(|_| io::Error::other("temporary-name randomness unavailable"))?;
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("file"), OsString::from);
    name.push(format!(".{}{TEMP_SUFFIX}", hex::encode(random)));
    Ok(path.with_file_name(name))
}

/// Writes `bytes` durably, atomically replacing an existing regular file.
pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, false)
}

/// Writes a secret file durably. Unix creates it as owner-readable/writable only;
/// Windows inherits the selected folder's ACL, which the UI tells the user to protect.
pub(crate) fn write_owner_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, true)
}

fn write_with(path: &Path, bytes: &[u8], owner_only: bool) -> io::Result<()> {
    ensure_final_file(path)?;
    let temp = temp_path(path)?;
    let result = (|| {
        let mut file = create_with_mode(&temp, owner_only)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Creates a new regular file exclusively. A pre-existing path, including a planted
/// symlink, is refused rather than truncated or followed.
pub(crate) fn create(path: &Path) -> io::Result<File> {
    create_with_mode(path, false)
}

fn create_with_mode(path: &Path, owner_only: bool) -> io::Result<File> {
    ensure_absent(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if owner_only {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = owner_only;
    let file = options.open(path)?;
    if !file.metadata()?.is_file() || !fs::symlink_metadata(path)?.file_type().is_file() {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(io::Error::other("created path is not a regular file"));
    }
    Ok(file)
}

/// Renames `from` to `to` and syncs the folder of `to`. Symlinks and special files at
/// either name are refused. An existing regular-file destination may be replaced.
pub(crate) fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let source = fs::symlink_metadata(from)?;
    if source.file_type().is_symlink() || !(source.is_file() || source.is_dir()) {
        return Err(io::Error::other(
            "rename source is not a regular file or directory",
        ));
    }
    match fs::symlink_metadata(to) {
        Ok(target) if target.file_type().is_symlink() => {
            return Err(io::Error::other("rename target is a symlink"));
        }
        Ok(target) if source.is_file() && !target.is_file() => {
            return Err(io::Error::other("rename target is not a regular file"));
        }
        Ok(_) if source.is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "rename target already exists",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(from, to)?;
    sync_parent(to)
}

fn ensure_absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary path already exists",
        )),
        Err(error) => Err(error),
    }
}

fn ensure_final_file(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(io::Error::other("destination is not a regular file")),
        Err(error) => Err(error),
    }
}

/// Makes a creation or rename in the folder holding `path` durable on Unix.
///
/// Known limitation of [`lis_store::sync_dir`]: on Windows a rename may be lost on power
/// failure. A copy is claimed only after it is read back, and a restore test detects a
/// lost one (M4 design brief E3).
pub(crate) fn sync_parent(path: &Path) -> io::Result<()> {
    path.parent().map_or(Ok(()), lis_store::sync_dir)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{MetadataExt, symlink};
    use std::path::Path;

    use tempfile::tempdir;

    use super::{temp_path, write_file, write_owner_file};

    #[test]
    fn unpredictable_temporary_names_differ() {
        let path = Path::new("kit.txt");
        assert_ne!(temp_path(path).unwrap(), temp_path(path).unwrap());
    }

    #[test]
    fn a_planted_final_symlink_is_refused() {
        let dir = tempdir().unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, b"safe").unwrap();
        let path = dir.path().join("kit.txt");
        symlink(&victim, &path).unwrap();
        assert!(write_file(&path, b"secret").is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"safe");
    }

    #[test]
    fn owner_file_has_mode_0600() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("kit.txt");
        write_owner_file(&path, b"secret").unwrap();
        assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o600);
    }
}
