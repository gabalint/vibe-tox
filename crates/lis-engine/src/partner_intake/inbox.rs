//! Reading an HL7 Orders connection's input folder, local or on the partner's SFTP server,
//! and moving each read file to Processed or Failed, by year and month, never replacing a
//! file: a name already taken gets `-1`, `-2`, … before its extension. Only complete
//! files are read: `*.hl7`, not hidden, not `*.tmp`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lis_auth::DataKey;
use lis_interfaces::partner::{Destination, MovedTo};
use lis_store::Database;

use crate::partner_delivery::sftp::{self, RemoteInbox};

/// Files read at most per connection and pass; the rest wait for the next pass.
pub(super) const MAX_FILES: usize = 20;
/// Names tried before giving up on a free one.
const MAX_SAME_NAME: u32 = 100;

/// One connection's input folder for the length of a pass.
pub(super) enum Inbox {
    Local { input: PathBuf, moved_to: MovedTo },
    Remote(Box<RemoteInbox>),
}

impl Inbox {
    /// Opens the folder, or signs in to the server. Blocking. The error is a PHI-free code.
    pub(super) fn open(
        db: &Database,
        key: Option<&DataKey>,
        destination: &Destination,
    ) -> Result<Self, &'static str> {
        let moved_to = destination.moved_to.clone().ok_or("not_incoming")?;
        let Some(server) = &destination.sftp else {
            return Ok(Self::Local {
                input: PathBuf::from(&destination.folder),
                moved_to,
            });
        };
        let credential =
            sftp::credential(db, key, server.auth, destination.sealed_password.as_deref())
                .map_err(|problem| problem.code())?;
        RemoteInbox::open(server, credential, moved_to)
            .map(|remote| Self::Remote(Box::new(remote)))
            .map_err(|problem| problem.code())
    }

    /// The complete files waiting, oldest name first, at most [`MAX_FILES`].
    pub(super) fn list(&mut self) -> Result<Vec<String>, &'static str> {
        let mut names = match self {
            Self::Local { input, .. } => local_names(input).map_err(|_| "folder_missing")?,
            Self::Remote(remote) => remote.names().map_err(|problem| problem.code())?,
        };
        names.retain(|name| complete(name));
        names.sort();
        names.truncate(MAX_FILES);
        Ok(names)
    }

    pub(super) fn read(&mut self, name: &str) -> Result<Vec<u8>, &'static str> {
        match self {
            Self::Local { input, .. } => fs::read(input.join(name)).map_err(|_| "read_failed"),
            Self::Remote(remote) => remote.read(name).map_err(|problem| problem.code()),
        }
    }

    /// Moves the file to Processed (or Failed) › `year` › `month` and returns where it went.
    pub(super) fn move_out(
        &mut self,
        name: &str,
        failed: bool,
        (year, month): (&str, &str),
    ) -> Result<String, &'static str> {
        match self {
            Self::Local { input, moved_to } => {
                let root = if failed {
                    &moved_to.failed
                } else {
                    &moved_to.processed
                };
                let folder = Path::new(root).join(year).join(month);
                move_local(&input.join(name), &folder, name).map_err(|_| "move_failed")
            }
            Self::Remote(remote) => remote
                .move_out(name, failed, year, month)
                .map_err(|problem| problem.code()),
        }
    }
}

/// `*.hl7` (any case), not hidden: a file still written as `*.tmp` is not complete.
pub(crate) fn complete(name: &str) -> bool {
    !name.starts_with('.')
        && Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("hl7"))
}

fn local_names(input: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(input)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && let Ok(name) = entry.file_name().into_string()
        {
            names.push(name);
        }
    }
    Ok(names)
}

/// The free name for `name` in `folder`: itself, else `stem-1.ext`, `stem-2.ext`, ….
pub(crate) fn candidate(name: &str, n: u32) -> String {
    if n == 0 {
        return name.to_owned();
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) => format!("{stem}-{n}.{extension}"),
        None => format!("{name}-{n}"),
    }
}

/// Links the file under a free name, then removes the original: an existing file is never
/// replaced. Across volumes it is copied, synced and then removed.
fn move_local(source: &Path, folder: &Path, name: &str) -> io::Result<String> {
    fs::create_dir_all(folder)?;
    for n in 0..MAX_SAME_NAME {
        let target = folder.join(candidate(name, n));
        let placed = match fs::hard_link(source, &target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => copy_new(source, &target),
        };
        match placed {
            Ok(()) => {
                fs::remove_file(source)?;
                lis_store::sync_dir(folder)?;
                return Ok(target.to_string_lossy().into_owned());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

fn copy_new(source: &Path, target: &Path) -> io::Result<()> {
    let bytes = fs::read(source)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    io::Write::write_all(&mut file, &bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_hl7_files_are_read() {
        assert!(complete("clinicA_000124.hl7"));
        assert!(complete("ORDER.HL7"));
        assert!(!complete("clinicA_000124.hl7.tmp"));
        assert!(!complete(".clinicA_000124.hl7"));
        assert!(!complete("clinicA_000124.txt"));
    }

    #[test]
    fn a_moved_file_never_replaces_another() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("2026").join("11");
        let input = root.path().join("in");
        fs::create_dir_all(&input).unwrap();
        for bytes in [b"first".as_slice(), b"second".as_slice()] {
            let source = input.join("a.hl7");
            fs::write(&source, bytes).unwrap();
            move_local(&source, &folder, "a.hl7").unwrap();
            assert!(!source.exists());
        }
        assert_eq!(fs::read(folder.join("a.hl7")).unwrap(), b"first");
        assert_eq!(fs::read(folder.join("a-1.hl7")).unwrap(), b"second");
    }
}
