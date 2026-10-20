//! Which installation the app opens, the laboratory's or the demo laboratory, and where each
//! keeps its files (proposed REQ-OPS-016; starter data brief, decision 1a).
//!
//! The demo's files sit beside the laboratory's data folder in the OS's local data folder,
//! so neither installation's folder is ever inside the other's, and nothing one
//! installation backs up, exports, logs or stores (the database and its artifact folder)
//! can include the other's files:
//!
//! | Path | Holds |
//! | --- | --- |
//! | `<local data>/<identifier>/` | The laboratory's data folder, as every earlier build used it |
//! | `<local data>/<identifier>.demo/` | The demo laboratory's data folder |
//! | `<local data>/<identifier>.demo-logs/` | The demo laboratory's PHI-free diagnostic logs |
//! | `<local data>/<identifier>.demo-deleted/` | A demo folder being deleted by a reset |
//! | `<local data>/<identifier>.installation` | The choice: `laboratory`, `demo` or `demo-reset` |
//!
//! The laboratory's logs stay where Tauri puts them (`~/Library/Logs/<identifier>` on
//! macOS, `<local data>/<identifier>/logs` on Windows and Linux); the demo's never go there.
//! Each has its own OS key-store entry: the bundle identifier, plus `.demo` for the demo.
//! The choice is read at start; a missing file means the laboratory. Resetting the demo is
//! recorded as `demo-reset` and done at the next start, before its engine opens anything:
//! [`Folders::reset_demo`] renames only the demo folder to the tombstone in one step, then
//! deletes the tombstone, so a demo is never opened half-deleted.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lis_engine::InstallationKind;

/// The demo folder's suffix, and the demo key-store entry's.
const DEMO_SUFFIX: &str = ".demo";
/// The demo log folder's suffix.
const DEMO_LOGS_SUFFIX: &str = ".demo-logs";
/// The suffix of a demo folder a reset is deleting.
const TOMBSTONE_SUFFIX: &str = ".demo-deleted";
/// How often a reset retries a rename or delete another process still blocks.
const RETRY_EVERY: Duration = Duration::from_millis(100);
/// The choice file's suffix.
const CHOICE_SUFFIX: &str = ".installation";

/// What the next start opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    /// The laboratory's own installation.
    Laboratory,
    /// The demo laboratory.
    Demo,
    /// The demo laboratory, after deleting its folder.
    DemoReset,
}

impl Choice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Laboratory => "laboratory",
            Self::Demo => "demo",
            Self::DemoReset => "demo-reset",
        }
    }

    /// The installation this choice opens.
    pub(crate) fn kind(self) -> InstallationKind {
        match self {
            Self::Laboratory => InstallationKind::Laboratory,
            Self::Demo | Self::DemoReset => InstallationKind::Demo,
        }
    }
}

/// The installation this run opened, managed as app state.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Installation(pub(crate) InstallationKind);

/// The choice file holds something this build does not know.
#[derive(Debug, thiserror::Error)]
#[error("the installation choice file is unreadable")]
pub(crate) struct ChoiceUnreadable;

/// Both installations' folders, derived from the laboratory's.
#[derive(Debug, Clone)]
pub(crate) struct Folders {
    laboratory: PathBuf,
    demo: PathBuf,
    tombstone: PathBuf,
    choice: PathBuf,
    laboratory_logs: PathBuf,
    demo_logs: PathBuf,
}

/// `<parent>/<name><suffix>`: a sibling of `path`, never inside it.
fn sibling(path: &Path, suffix: &str) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    Some(path.parent()?.join(format!("{name}{suffix}")))
}

impl Folders {
    /// The folders for the laboratory data folder `laboratory` and log folder `logs`, as the
    /// OS resolves them for this bundle; `None` when the data folder has no parent or name.
    pub(crate) fn new(laboratory: PathBuf, logs: PathBuf) -> Option<Self> {
        Some(Self {
            demo: sibling(&laboratory, DEMO_SUFFIX)?,
            tombstone: sibling(&laboratory, TOMBSTONE_SUFFIX)?,
            choice: sibling(&laboratory, CHOICE_SUFFIX)?,
            demo_logs: sibling(&laboratory, DEMO_LOGS_SUFFIX)?,
            laboratory,
            laboratory_logs: logs,
        })
    }

    /// The data folder of `kind`.
    pub(crate) fn data_dir(&self, kind: InstallationKind) -> &Path {
        match kind {
            InstallationKind::Laboratory => &self.laboratory,
            InstallationKind::Demo => &self.demo,
        }
    }

    /// The log folder of `kind`.
    pub(crate) fn log_dir(&self, kind: InstallationKind) -> &Path {
        match kind {
            InstallationKind::Laboratory => &self.laboratory_logs,
            InstallationKind::Demo => &self.demo_logs,
        }
    }

    /// What the next start opens: the laboratory when nothing was chosen.
    pub(crate) fn chosen(&self) -> Result<Choice, ChoiceUnreadable> {
        let text = match fs::read_to_string(&self.choice) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Choice::Laboratory);
            }
            Err(_) => return Err(ChoiceUnreadable),
        };
        [Choice::Laboratory, Choice::Demo, Choice::DemoReset]
            .into_iter()
            .find(|choice| choice.as_str() == text.trim())
            .ok_or(ChoiceUnreadable)
    }

    /// Records what the next start opens, replacing the file in one rename.
    pub(crate) fn choose(&self, choice: Choice) -> io::Result<()> {
        if let Some(parent) = self.choice.parent() {
            fs::create_dir_all(parent)?;
        }
        let partial = self.choice.with_extension("installation-new");
        {
            // Sync through the writing handle: Windows refuses to flush a read-only one,
            // and the handle must be closed before the rename.
            let mut file = fs::File::create(&partial)?;
            file.write_all(format!("{}\n", choice.as_str()).as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&partial, &self.choice)
    }

    /// Empties the demo laboratory, and nothing else. Refused unless the demo folder and
    /// the tombstone are the laboratory folder's own `.demo` and `.demo-deleted` siblings,
    /// so neither can be the laboratory folder, inside it or around it. An earlier tombstone
    /// is deleted first; then the demo folder is renamed to the tombstone in one step, so
    /// it is either whole or gone, and the tombstone is deleted. Symbolic links are renamed
    /// or removed, never followed. While another process still holds files (the previous
    /// run, on Windows), each step retries for up to `wait`. `Ok(false)` means the demo is
    /// gone but the tombstone remains; [`remove_tombstone`](Self::remove_tombstone) at a
    /// later start finishes it.
    pub(crate) fn reset_demo(&self, wait: Duration) -> io::Result<bool> {
        if !is_sibling(&self.laboratory, &self.demo, DEMO_SUFFIX)
            || !is_sibling(&self.laboratory, &self.tombstone, TOMBSTONE_SUFFIX)
        {
            return Err(io::Error::other("not the demo laboratory's folder"));
        }
        retrying(wait, || remove(&self.tombstone))?;
        match retrying(wait, || fs::rename(&self.demo, &self.tombstone)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            renamed => renamed?,
        }
        Ok(retrying(wait, || remove(&self.tombstone)).is_ok())
    }

    /// Deletes a tombstone an interrupted reset left, if any; the demo folder is untouched.
    pub(crate) fn remove_tombstone(&self) -> io::Result<()> {
        if !is_sibling(&self.laboratory, &self.tombstone, TOMBSTONE_SUFFIX) {
            return Err(io::Error::other("not the demo laboratory's tombstone"));
        }
        remove(&self.tombstone)
    }
}

/// `path` is `<laboratory><suffix>` beside `laboratory`, and neither contains the other.
fn is_sibling(laboratory: &Path, path: &Path, suffix: &str) -> bool {
    sibling(laboratory, suffix).as_deref() == Some(path)
        && !laboratory.starts_with(path)
        && !path.starts_with(laboratory)
}

/// Removes `path` and everything in it; a symbolic link is removed, not followed. An absent
/// path is fine.
fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

/// Runs `step` until it succeeds, it reports `NotFound`, or `wait` has passed.
fn retrying(wait: Duration, mut step: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let started = Instant::now();
    loop {
        match step() {
            Err(error) if error.kind() != io::ErrorKind::NotFound && started.elapsed() < wait => {
                std::thread::sleep(RETRY_EVERY);
            }
            done => return done,
        }
    }
}

/// The OS key-store entry of `kind`: the bundle identifier for the laboratory, so an
/// existing installation keeps its key, and a separate entry for the demo.
pub(crate) fn key_identifier(identifier: &str, kind: InstallationKind) -> String {
    match kind {
        InstallationKind::Laboratory => identifier.to_owned(),
        InstallationKind::Demo => format!("{identifier}{DEMO_SUFFIX}"),
    }
}

#[cfg(test)]
mod tests;
