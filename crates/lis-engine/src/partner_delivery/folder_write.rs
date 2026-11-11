//! Writing a message file into a partner's folder so the partner never reads
//! a partial file and nothing there is replaced: the bytes go to a uniquely
//! named `.tmp` file, are synced, and only then get their final name. A name
//! already taken by other content gets `-1`, `-2`, … before the extension; a
//! name already holding these exact bytes is the earlier write of the same
//! delivery (after a crash before it was recorded) and is kept as it is.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use lis_core::Id;
use lis_interfaces::partner::MovedTo;

/// Names tried before giving up on a free one.
const MAX_SAME_NAME: u32 = 100;

/// Why a file was not written; the codes are PHI-free and shown in Logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteProblem {
    /// The folder does not exist or cannot be reached.
    FolderMissing,
    /// This computer may not write there.
    AccessDenied,
    /// The disk is full.
    DiskFull,
    /// Every candidate name is taken.
    NameTaken,
    /// Any other file system failure.
    WriteFailed,
}

impl WriteProblem {
    /// The stored and displayed code.
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::FolderMissing => "folder_missing",
            Self::AccessDenied => "access_denied",
            Self::DiskFull => "disk_full",
            Self::NameTaken => "name_taken",
            Self::WriteFailed => "write_failed",
        }
    }

    fn from_io(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => Self::FolderMissing,
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                Self::AccessDenied
            }
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => Self::DiskFull,
            _ => Self::WriteFailed,
        }
    }
}

/// Writes `bytes` as `name` in `folder` and returns the path it got.
pub(crate) fn write_file(folder: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, WriteProblem> {
    ensure_folder(folder)?;
    let temporary = folder.join(format!("{name}.{}.tmp", random_suffix()));
    fill(&temporary, bytes).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        WriteProblem::from_io(&error)
    })?;
    let placed = place(folder, name, &temporary, bytes);
    // The temporary name is gone after a rename; otherwise remove it now.
    let _ = fs::remove_file(&temporary);
    let target = placed?;
    lis_store::sync_dir(folder).map_err(|error| WriteProblem::from_io(&error))?;
    Ok(target)
}

/// Writes a small probe file, syncs it and removes it: whether this computer
/// can create, write and delete files in `folder`.
pub(crate) fn probe(folder: &Path) -> Result<(), WriteProblem> {
    ensure_folder(folder)?;
    let probe = folder.join(format!(".vibetox-probe-{}.tmp", random_suffix()));
    let written = fill(&probe, b"probe\n");
    let removed = fs::remove_file(&probe);
    written
        .and(removed)
        .map_err(|error| WriteProblem::from_io(&error))
}

/// Whether this computer can read the input folder and create, write and delete files in
/// the Processed and Failed folders: an incoming connection's folders.
pub(crate) fn probe_intake(input: &Path, moved_to: &MovedTo) -> Result<(), WriteProblem> {
    ensure_folder(input)?;
    fs::read_dir(input).map_err(|error| WriteProblem::from_io(&error))?;
    probe(Path::new(&moved_to.processed))?;
    probe(Path::new(&moved_to.failed))
}

fn ensure_folder(folder: &Path) -> Result<(), WriteProblem> {
    match fs::metadata(folder) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(WriteProblem::FolderMissing),
        Err(error) => Err(WriteProblem::from_io(&error)),
    }
}

fn fill(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file: File = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Gives the synced temporary file its final name without replacing a file.
fn place(
    folder: &Path,
    name: &str,
    temporary: &Path,
    bytes: &[u8],
) -> Result<PathBuf, WriteProblem> {
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    for n in 0..MAX_SAME_NAME {
        let candidate = if n == 0 {
            name.to_owned()
        } else {
            format!("{stem}-{n}.{extension}")
        };
        let target = folder.join(candidate);
        match fs::read(&target) {
            Ok(existing) if existing == bytes => return Ok(target),
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(WriteProblem::from_io(&error)),
        }
        match fs::hard_link(temporary, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            // Some shares do not support links; the name was free a moment ago.
            Err(_) => {
                return fs::rename(temporary, &target)
                    .map(|()| target)
                    .map_err(|error| WriteProblem::from_io(&error));
            }
        }
    }
    Err(WriteProblem::NameTaken)
}

pub(super) fn random_suffix() -> String {
    let id: Id<()> = Id::generate();
    id.to_string().replace('-', "")[20..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_appears_only_under_its_final_name() {
        let folder = tempfile::tempdir().unwrap();
        let path = write_file(folder.path(), "billing-1.json", b"{}\n").unwrap();
        assert_eq!(path, folder.path().join("billing-1.json"));
        let names: Vec<_> = fs::read_dir(folder.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["billing-1.json"]);
    }

    #[test]
    fn nothing_is_replaced_and_a_rewrite_is_recognized() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("billing-1.json"), b"other").unwrap();
        let first = write_file(folder.path(), "billing-1.json", b"{}\n").unwrap();
        assert_eq!(first, folder.path().join("billing-1-1.json"));
        assert_eq!(
            fs::read(folder.path().join("billing-1.json")).unwrap(),
            b"other"
        );
        let again = write_file(folder.path(), "billing-1.json", b"{}\n").unwrap();
        assert_eq!(again, first);
    }

    #[test]
    fn a_missing_folder_is_reported() {
        let folder = tempfile::tempdir().unwrap();
        let missing = folder.path().join("gone");
        assert_eq!(
            write_file(&missing, "billing-1.json", b"{}"),
            Err(WriteProblem::FolderMissing)
        );
        assert_eq!(probe(&missing), Err(WriteProblem::FolderMissing));
        assert_eq!(probe(folder.path()), Ok(()));
    }
}
