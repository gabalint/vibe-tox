//! The file-system side of the poller (§6): which files are candidates, reading one
//! that has not changed, and modification times. Never follows a link.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use lis_core::UtcTimestamp;
use lis_interfaces::MAX_RESULT_FILE_BYTES;

/// A file in a watched folder that may be a result file.
pub(crate) struct Candidate {
    pub(crate) name: String,
    pub(crate) byte_len: u64,
    pub(crate) modified_at: UtcTimestamp,
}

/// The candidates directly in `folder` (§6); an error when the folder cannot be listed.
pub(crate) fn candidates(folder: &Path) -> io::Result<Vec<Candidate>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder)? {
        let Ok(entry) = entry else {
            continue;
        };
        // A name that is not Unicode cannot be stored as received; it is skipped.
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !candidate_name(&name) {
            continue;
        }
        // Not followed: a link has its own type, which is not a regular file.
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.file_type().is_file() || hidden(&metadata) {
            continue;
        }
        if let Some(modified_at) = modified_at(&metadata) {
            found.push(Candidate {
                name,
                byte_len: metadata.len(),
                modified_at,
            });
        }
    }
    Ok(found)
}

/// `.csv` or `.txt` in any case, not hidden by a leading dot, not an Office lock file.
/// `*.tmp` and `*.part` files fail the suffix rule.
fn candidate_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.starts_with('.')
        && !name.starts_with("~$")
        && (lower.ends_with(".csv") || lower.ends_with(".txt"))
}

#[cfg(windows)]
fn hidden(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn hidden(_metadata: &Metadata) -> bool {
    // A leading dot is the convention, checked with the name.
    false
}

/// Opens a result file to read it. On Windows only other readers may share it, so an
/// exporter that still holds the file for writing makes the open fail with a sharing
/// violation, and the file waits (§6).
pub(crate) fn open_for_reading(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        options.share_mode(FILE_SHARE_READ);
    }
    options.open(path)
}

/// The bytes of a file whose size and time still match `candidate` after reading.
pub(super) fn read_unchanged(path: &Path, candidate: &Candidate) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    open_for_reading(path)
        .ok()?
        .take(MAX_RESULT_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    let after = fs::symlink_metadata(path).ok()?;
    let unchanged = after.file_type().is_file()
        && after.len() == candidate.byte_len
        && u64::try_from(bytes.len()).ok() == Some(candidate.byte_len)
        && modified_at(&after) == Some(candidate.modified_at);
    unchanged.then_some(bytes)
}

/// The file's modification time; `None` when the file system has none or it is before 1970.
pub(crate) fn modified_at(metadata: &Metadata) -> Option<UtcTimestamp> {
    utc(metadata.modified().ok()?)
}

pub(super) fn utc(time: SystemTime) -> Option<UtcTimestamp> {
    let since_epoch = time.duration_since(UNIX_EPOCH).ok()?;
    UtcTimestamp::parse("1970-01-01T00:00:00.000000000Z")
        .ok()?
        .checked_add(since_epoch)
}
