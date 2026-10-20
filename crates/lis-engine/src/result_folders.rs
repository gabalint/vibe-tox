//! The watched-folder poller (REQ-INT-009, REQ-OPS-013; result-files brief §6, F13, F14).
//!
//! From engine start, whoever is signed in (also nobody), the poller lists the folder of
//! every enabled result-file instrument that has one, now and every [`SCAN_INTERVAL`], and
//! receives each new file once it is complete:
//!
//! - **Candidates** are regular files directly in the folder whose name ends in `.csv` or
//!   `.txt`, in any case. Subfolders, links, hidden files and `~$*` files are skipped;
//!   `*.tmp` and `*.part` files fail the suffix rule.
//! - **Ready** means the same size and modification time at two scans at least
//!   10 seconds apart, by the engine clock. On Windows the file must also open with
//!   sharing that refuses writers. Size and time are checked again after reading.
//! - **Already received** files are in the ledger by name, size and time, so a file is read
//!   once and the same name with new content is read again. Files present when the folder
//!   was chosen are skipped the same way (§6, "Present at selection"). Files are never
//!   moved or deleted.
//! - **Receipt.** The instrument is read first: still enabled, still result files, still
//!   watching this folder. A file over the size limit becomes an aborted receipt without
//!   being read, logged as a warning and counted for the instrument's health until the app
//!   restarts. Otherwise its exact bytes go into the artifact store first; then the receipt
//!   is written as the instrument's principal, in a transaction that checks the folder
//!   again, and the receipt queues the import. A receipt that fails is tried again after
//!   10 seconds, doubling up to 5 minutes per file.
//!
//! Each instrument's state goes to the listener status, so the sign-in screen, the
//! keep-awake controller and the `listeners` health check treat a watched folder like a
//! listener: listening, or in error while the folder is missing or unreadable, with the
//! counts of files waiting longer than 5 minutes and of files too large. An enabled
//! instrument without a folder is manual loading only. Logs carry instrument IDs and
//! codes, never file names or paths.

mod files;
mod intake;
mod poller;
mod state;

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lis_core::{Clock, InstrumentId, UtcTimestamp};
use lis_interfaces::{
    Instrument, ListenerDetailCode, ListenerState, ListenerSupervisor, StatusReport,
};
use lis_store::Database;

use self::files::Candidate;
pub(crate) use self::files::{candidates, modified_at, open_for_reading};
use self::intake::Taken;
pub(crate) use self::poller::FolderPoller;
use self::state::PollState;
use crate::ops_worker::locked;

/// How often the folders are listed.
const SCAN_INTERVAL: Duration = Duration::from_secs(10);

/// The poller's shared part: what a scan needs and remembers between scans.
pub(crate) struct ResultFolders {
    db: Arc<Database>,
    clock: Arc<dyn Clock>,
    listeners: Arc<ListenerSupervisor>,
    /// Raised at shutdown; a scan stops between files.
    cancel: AtomicBool,
    state: Mutex<PollState>,
}

impl ResultFolders {
    /// The poller of the engine: shared state and its running loop, which scans at once.
    /// Must run inside a tokio runtime.
    pub(crate) fn start(
        db: &Arc<Database>,
        clock: &Arc<dyn Clock>,
        listeners: &Arc<ListenerSupervisor>,
    ) -> (Arc<Self>, FolderPoller) {
        let folders = Arc::new(Self::new(
            Arc::clone(db),
            Arc::clone(clock),
            Arc::clone(listeners),
        ));
        let poller = FolderPoller::start(Arc::clone(&folders), SCAN_INTERVAL);
        (folders, poller)
    }

    pub(crate) fn new(
        db: Arc<Database>,
        clock: Arc<dyn Clock>,
        listeners: Arc<ListenerSupervisor>,
    ) -> Self {
        Self {
            db,
            clock,
            listeners,
            cancel: AtomicBool::new(false),
            state: Mutex::new(PollState::default()),
        }
    }

    /// One scan of every result-file instrument's folder. Blocking: file system and
    /// database work, so it runs off the async runtime.
    pub(crate) fn scan(&self) {
        let instruments = match self
            .db
            .read_blocking(lis_interfaces::result_file_instruments)
        {
            Ok(instruments) => instruments,
            Err(error) => {
                tracing::warn!(
                    error_code = error.code().as_str(),
                    "result folders not read"
                );
                return;
            }
        };
        let now = self.clock.now();
        let mut state = locked(&self.state);
        let watched: HashSet<InstrumentId> = instruments
            .iter()
            .filter(|instrument| instrument.enabled && instrument.result_folder.is_some())
            .map(|instrument| instrument.id)
            .collect();
        state.pending.retain(|(id, _), _| watched.contains(id));
        for instrument in instruments {
            if self.cancel.load(Ordering::Acquire) {
                return;
            }
            let report = self.scan_instrument(&mut state, &instrument, now);
            self.report(&mut state, instrument.id, report);
        }
    }

    /// The instrument's folder scanned, or why it is not watched: disabled, or
    /// enabled without a folder, when its files are loaded by hand (§6).
    fn scan_instrument(
        &self,
        state: &mut PollState,
        instrument: &Instrument,
        now: UtcTimestamp,
    ) -> StatusReport {
        let unwatched = |state, detail_code| StatusReport {
            state,
            detail_code,
            files_waiting: None,
            files_too_large: None,
        };
        if !instrument.enabled {
            return unwatched(
                ListenerState::Disabled,
                Some(ListenerDetailCode::ConfigurationDisabled),
            );
        }
        match &instrument.result_folder {
            Some(folder) => self.scan_folder(state, instrument.id, folder, now),
            None => unwatched(ListenerState::ManualOnly, None),
        }
    }

    /// Stops a scan in progress between files, for shutdown.
    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    fn scan_folder(
        &self,
        state: &mut PollState,
        instrument_id: InstrumentId,
        folder: &str,
        now: UtcTimestamp,
    ) -> StatusReport {
        let Ok(candidates) = candidates(Path::new(folder)) else {
            return StatusReport {
                state: ListenerState::Error,
                detail_code: Some(ListenerDetailCode::FolderUnavailable),
                files_waiting: Some(state.waiting(instrument_id, now)),
                files_too_large: Some(state.too_large(instrument_id)),
            };
        };
        // Forget files that left the folder.
        let names: HashSet<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
        state
            .pending
            .retain(|(id, name), _| *id != instrument_id || names.contains(name.as_str()));
        state
            .received
            .retain(|(id, name, ..)| *id != instrument_id || names.contains(name.as_str()));
        for candidate in candidates {
            if self.cancel.load(Ordering::Acquire) {
                break;
            }
            self.consider(state, instrument_id, folder, candidate, now);
        }
        let (waiting, too_large) = (
            state.waiting(instrument_id, now),
            state.too_large(instrument_id),
        );
        let detail_code = if waiting > 0 {
            Some(ListenerDetailCode::FilesWaiting)
        } else {
            (too_large > 0).then_some(ListenerDetailCode::FileTooLarge)
        };
        StatusReport {
            state: ListenerState::Listening,
            detail_code,
            files_waiting: Some(waiting),
            files_too_large: Some(too_large),
        }
    }

    fn consider(
        &self,
        state: &mut PollState,
        instrument_id: InstrumentId,
        folder: &str,
        candidate: Candidate,
        now: UtcTimestamp,
    ) {
        let key = (
            instrument_id,
            candidate.name.clone(),
            candidate.byte_len,
            candidate.modified_at,
        );
        let pending_key = (instrument_id, candidate.name.clone());
        if state.received.contains(&key) || self.known(instrument_id, folder, &candidate) {
            state.pending.remove(&pending_key);
            state.received.insert(key);
            return;
        }
        if !state.observe(instrument_id, &candidate, now) {
            return;
        }
        match self.take(instrument_id, folder, &candidate) {
            Taken::Received { too_large } => {
                state.pending.remove(&pending_key);
                state.received.insert(key);
                if too_large {
                    // F14: not silent; the receipt records it, health shows it until restart.
                    tracing::warn!(
                        instrument_id = %instrument_id,
                        error_code = "file_too_large",
                        "result file over the size limit; not read"
                    );
                    *state.too_large.entry(instrument_id).or_default() += 1;
                }
            }
            Taken::Changed => {
                if let Some(pending) = state.pending.get_mut(&pending_key) {
                    pending.unchanged_since = now;
                }
            }
            outcome @ (Taken::Retry(code) | Taken::Refused(code)) => {
                let Some(pending) = state.pending.get_mut(&pending_key) else {
                    return;
                };
                pending.refused = matches!(outcome, Taken::Refused(_));
                if !pending.refused {
                    pending.back_off(now);
                }
                if !pending.failing {
                    tracing::warn!(
                        instrument_id = %instrument_id,
                        error_code = code,
                        retried = !pending.refused,
                        "result file not received"
                    );
                }
                pending.failing = true;
            }
        }
    }

    /// Publishes the state to the listener status, logging changes only.
    fn report(&self, state: &mut PollState, instrument_id: InstrumentId, report: StatusReport) {
        let summary = (report.state, report.detail_code);
        if state.reported.insert(instrument_id, summary) != Some(summary) {
            tracing::info!(
                instrument_id = %instrument_id,
                state = ?report.state,
                detail = ?report.detail_code,
                files_waiting = report.files_waiting,
                files_too_large = report.files_too_large,
                "result folder state changed"
            );
        }
        self.listeners.report_folder_status(instrument_id, report);
    }
}

#[cfg(test)]
mod tests;
