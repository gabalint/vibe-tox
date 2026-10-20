//! The watched-folder poller on a real database and temporary folders (F13, F14, §6).
//! The engine clock is fixed and moved by hand, so stability and waiting times are exact.
//! Synthetic files only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use lis_core::Phi;
use lis_core::{Capability, Clock, CorrelationId, FixedClock, Principal, UtcTimestamp};
use lis_interfaces::{
    ConnectionSettings, Instrument, InterfaceError, ListenerDetailCode, ListenerState,
    ListenerStatus, ListenerSupervisor, MAX_RESULT_FILE_BYTES, PresentFile, RegisterInstrument,
    SetInstrumentEnabled, SetResultFolder, WatchedFolder,
};
use lis_store::Database;
use tempfile::TempDir;

use super::files::utc;
use super::*;

const EXPORT: &[u8] = b"Sample Name,Component,Final Conc\nC00000123,Morphine,152.4\n";

struct Poll {
    db: Arc<Database>,
    clock: Arc<FixedClock>,
    listeners: Arc<ListenerSupervisor>,
    folders: ResultFolders,
    instrument: Instrument,
    folder: PathBuf,
    data: TempDir,
    _watched: TempDir,
}

fn admin() -> Principal {
    Principal::system("test-admin", [Capability::ManageConfiguration])
}

impl Poll {
    /// An enabled result-file instrument watching an empty temporary folder.
    fn new() -> Self {
        let data = tempfile::tempdir().unwrap();
        let watched = tempfile::tempdir().unwrap();
        let clock = Arc::new(FixedClock::new(
            UtcTimestamp::parse("2026-10-13T08:00:00.000000000Z").unwrap(),
        ));
        let db = Arc::new(
            Database::open(data.path(), &[lis_interfaces::SCHEMA], clock.clone()).unwrap(),
        );
        let folder = fs::canonicalize(watched.path()).unwrap();
        let registered = Self::configure(&db, |ctx| {
            lis_interfaces::register_instrument(
                ctx,
                RegisterInstrument {
                    code: "LCMS-1".to_owned(),
                    name: "Synthetic LC-MS/MS".to_owned(),
                    connection: ConnectionSettings::ResultFile {},
                    enabled: true,
                },
            )
        });
        let path = folder.to_str().unwrap().to_owned();
        let instrument = Self::configure(&db, move |ctx| {
            lis_interfaces::set_result_folder(
                ctx,
                SetResultFolder {
                    instrument_id: registered.id,
                    expected_revision: registered.revision,
                    folder: Some(WatchedFolder {
                        path,
                        present: Vec::new(),
                    }),
                },
            )
        });
        let listeners = Arc::new(ListenerSupervisor::new(Arc::clone(&db)));
        let folders = ResultFolders::new(Arc::clone(&db), clock.clone(), Arc::clone(&listeners));
        Self {
            db,
            clock,
            listeners,
            folders,
            instrument,
            folder,
            data,
            _watched: watched,
        }
    }

    fn configure<T: Send + 'static>(
        db: &Database,
        command: impl FnOnce(&mut lis_store::CommandContext<'_>) -> Result<T, InterfaceError>
        + Send
        + 'static,
    ) -> T {
        db.write_blocking(admin(), CorrelationId::generate(), command)
            .unwrap()
    }

    fn advance(&self, seconds: u64) {
        let now = self.clock.now();
        self.clock
            .set(now.checked_add(Duration::from_secs(seconds)).unwrap());
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        fs::write(self.folder.join(name), bytes).unwrap();
    }

    /// `(state, abort reason, file name)` of every receipt, oldest first.
    fn receipts(&self) -> Vec<(String, Option<String>, String)> {
        self.db
            .read_blocking(|conn| {
                let mut statement = conn.prepare(
                    "SELECT r.state, r.abort_reason, f.file_name FROM ifc_receipts r
                     JOIN ifc_receipt_files f ON f.receipt_id = r.id ORDER BY r.rowid",
                )?;
                let rows = statement
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok::<_, InterfaceError>(rows)
            })
            .unwrap()
    }

    fn status(&self) -> ListenerStatus {
        self.listeners
            .status_snapshot()
            .into_iter()
            .find(|status| status.instrument_id == self.instrument.id)
            .unwrap()
    }

    /// Scans, then scans again once the stability time has passed.
    fn scan_twice(&self) {
        self.folders.scan();
        self.advance(10);
        self.folders.scan();
    }
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = candidates(folder)
        .unwrap()
        .into_iter()
        .map(|candidate| candidate.name)
        .collect();
    names.sort();
    names
}

// §6: regular `.csv` and `.txt` files directly in the folder; no subfolders, links,
// hidden, Office lock, temporary or partial files.
#[test]
fn candidates_are_result_files_directly_in_the_folder() {
    let poll = Poll::new();
    for name in [
        "batch-1.csv",
        "BATCH-2.TXT",
        "batch-3.tmp",
        "batch-4.csv.part",
        ".batch-5.csv",
        "~$batch-6.csv",
        "batch-7.json",
    ] {
        poll.put(name, EXPORT);
    }
    fs::create_dir(poll.folder.join("subfolder.csv")).unwrap();
    fs::write(poll.folder.join("subfolder.csv").join("inner.csv"), EXPORT).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        poll.folder.join("batch-1.csv"),
        poll.folder.join("link.csv"),
    )
    .unwrap();
    assert_eq!(names(&poll.folder), ["BATCH-2.TXT", "batch-1.csv"]);
}

// F13: a file is read once its size and time are unchanged across two scans at least
// 10 seconds apart, and only once.
#[test]
fn a_file_is_read_once_unchanged_across_two_scans_ten_seconds_apart() {
    let poll = Poll::new();
    poll.put("batch-1.csv", EXPORT);
    poll.folders.scan();
    assert_eq!(poll.status().state, ListenerState::Listening);
    poll.advance(9);
    poll.folders.scan();
    assert!(poll.receipts().is_empty(), "read before it was stable");
    poll.advance(1);
    poll.folders.scan();
    assert_eq!(
        poll.receipts(),
        [("complete".to_owned(), None, "batch-1.csv".to_owned())]
    );
    poll.advance(60);
    poll.folders.scan();
    assert_eq!(
        poll.receipts().len(),
        1,
        "a received file is not read again"
    );
    assert_eq!(
        (poll.status().detail_code, poll.status().files_waiting),
        (None, Some(0))
    );
}

// §6: a file still being written waits until it stops changing.
#[test]
fn a_growing_file_waits_until_it_stops_changing() {
    let poll = Poll::new();
    poll.put("batch-1.csv", &EXPORT[..20]);
    poll.folders.scan();
    poll.advance(10);
    poll.put("batch-1.csv", EXPORT);
    poll.folders.scan();
    assert!(poll.receipts().is_empty());
    poll.advance(10);
    poll.folders.scan();
    assert_eq!(poll.receipts().len(), 1);
    let len: i64 = poll
        .db
        .read_blocking(|conn| {
            Ok::<_, InterfaceError>(conn.query_row(
                "SELECT byte_len FROM ifc_receipt_files",
                [],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(len, i64::try_from(EXPORT.len()).unwrap());
}

// F13: the ledger survives a restart, so a new run skips files already received.
#[test]
fn a_new_run_skips_files_already_received() {
    let poll = Poll::new();
    poll.put("batch-1.csv", EXPORT);
    poll.scan_twice();
    let restarted = ResultFolders::new(
        Arc::clone(&poll.db),
        poll.clock.clone(),
        Arc::clone(&poll.listeners),
    );
    restarted.scan();
    poll.advance(10);
    restarted.scan();
    assert_eq!(poll.receipts().len(), 1);
}

// F14: a file over the limit is an aborted receipt, not read and not stored.
#[test]
fn an_oversize_file_is_recorded_without_being_read() {
    let poll = Poll::new();
    let file = fs::File::create(poll.folder.join("huge.csv")).unwrap();
    file.set_len(MAX_RESULT_FILE_BYTES + 1).unwrap();
    poll.scan_twice();
    assert_eq!(
        poll.receipts(),
        [(
            "aborted".to_owned(),
            Some("file_too_large".to_owned()),
            "huge.csv".to_owned()
        )]
    );
    assert!(poll.db.artifacts().digests().unwrap().digests.is_empty());
    poll.advance(10);
    poll.folders.scan();
    assert_eq!(poll.receipts().len(), 1, "recorded once");
}

// §6 health: a missing or unreadable folder is an error until it is back.
#[test]
fn a_missing_folder_is_an_error_until_it_returns() {
    let poll = Poll::new();
    fs::remove_dir(&poll.folder).unwrap();
    poll.folders.scan();
    let status = poll.status();
    assert_eq!(
        (status.state, status.detail_code),
        (
            ListenerState::Error,
            Some(ListenerDetailCode::FolderUnavailable)
        )
    );
    assert!(status.in_error());
    fs::create_dir(&poll.folder).unwrap();
    poll.put("batch-1.csv", EXPORT);
    poll.scan_twice();
    assert_eq!(poll.status().state, ListenerState::Listening);
    assert_eq!(poll.receipts().len(), 1);
}

// §6 health: a file that has waited more than 5 minutes is counted until it is read.
#[test]
fn a_file_waiting_more_than_five_minutes_raises_the_signal() {
    let poll = Poll::new();
    let mut content = EXPORT.to_vec();
    poll.put("batch-1.csv", &content);
    poll.folders.scan();
    for _ in 0..30 {
        poll.advance(10);
        content.push(b'\n');
        poll.put("batch-1.csv", &content);
        poll.folders.scan();
    }
    assert_eq!(poll.status().detail_code, None, "5 minutes is not yet late");
    poll.advance(10);
    content.push(b'\n');
    poll.put("batch-1.csv", &content);
    poll.folders.scan();
    let status = poll.status();
    assert_eq!(
        (status.state, status.detail_code, status.files_waiting),
        (
            ListenerState::Listening,
            Some(ListenerDetailCode::FilesWaiting),
            Some(1)
        )
    );
    // A counted warning for health, not a folder in error (wave-10 review L10).
    assert!(!status.in_error());
    poll.advance(10);
    poll.folders.scan();
    assert_eq!(poll.receipts().len(), 1);
    assert_eq!(poll.status().detail_code, None);
}

// F13: a disabled instrument's folder is not watched.
#[test]
fn a_disabled_instrument_is_not_watched() {
    let poll = Poll::new();
    let instrument = poll.instrument.clone();
    Poll::configure(&poll.db, move |ctx| {
        lis_interfaces::set_instrument_enabled(
            ctx,
            SetInstrumentEnabled {
                instrument_id: instrument.id,
                expected_revision: instrument.revision,
                enabled: false,
            },
        )
    });
    poll.put("batch-1.csv", EXPORT);
    poll.scan_twice();
    assert!(poll.receipts().is_empty());
    let status = poll.status();
    assert_eq!(
        (status.state, status.detail_code),
        (
            ListenerState::Disabled,
            Some(ListenerDetailCode::ConfigurationDisabled)
        )
    );
}

// L10 of the wave-10 review: an enabled instrument without a folder is manual loading
// only, not disabled.
#[test]
fn an_enabled_instrument_without_a_folder_is_manual_loading_only() {
    let poll = Poll::new();
    let instrument = poll.instrument.clone();
    Poll::configure(&poll.db, move |ctx| {
        lis_interfaces::set_result_folder(
            ctx,
            SetResultFolder {
                instrument_id: instrument.id,
                expected_revision: instrument.revision,
                folder: None,
            },
        )
    });
    poll.folders.scan();
    let status = poll.status();
    assert_eq!(
        (status.state, status.detail_code, status.files_waiting),
        (ListenerState::ManualOnly, None, None)
    );
    assert!(!status.in_error());
}

// M3 of the wave-10 review: an oversize file is not silent: the instrument's status
// counts it until the app restarts, while its receipt stays.
#[test]
fn an_oversize_file_is_counted_in_the_status_until_restart() {
    let poll = Poll::new();
    let file = fs::File::create(poll.folder.join("huge.csv")).unwrap();
    file.set_len(MAX_RESULT_FILE_BYTES + 1).unwrap();
    poll.scan_twice();
    let status = poll.status();
    assert_eq!(
        (status.state, status.detail_code, status.files_too_large),
        (
            ListenerState::Listening,
            Some(ListenerDetailCode::FileTooLarge),
            Some(1)
        )
    );
    assert!(!status.in_error());
    poll.advance(60);
    poll.folders.scan();
    assert_eq!(poll.status().files_too_large, Some(1), "kept while running");
    let restarted = ResultFolders::new(
        Arc::clone(&poll.db),
        poll.clock.clone(),
        Arc::clone(&poll.listeners),
    );
    restarted.scan();
    let status = poll.status();
    assert_eq!(
        (status.detail_code, status.files_too_large),
        (None, Some(0))
    );
    assert_eq!(poll.receipts().len(), 1, "the aborted receipt stays");
}

// L6 of the wave-10 review: a file whose receipt keeps failing is tried again after
// 10 seconds, then 20, doubling, not at every scan.
#[test]
fn a_failing_receipt_backs_off_and_doubles() {
    let poll = Poll::new();
    let artifacts = poll.data.path().join(lis_store::ARTIFACTS_DIR);
    // A file where the artifact folder was: every put fails until it is back.
    let break_store = || {
        fs::remove_dir_all(&artifacts).unwrap();
        fs::write(&artifacts, b"").unwrap();
    };
    let repair_store = || {
        fs::remove_file(&artifacts).unwrap();
        fs::create_dir(&artifacts).unwrap();
    };
    break_store();
    poll.put("batch-1.csv", EXPORT);
    poll.scan_twice(); // t = 10: first failure; next try at 20.
    poll.advance(10);
    poll.folders.scan(); // t = 20: second failure; next try at 40.
    repair_store();
    for _ in 0..19 {
        poll.advance(1);
        poll.folders.scan();
    }
    assert!(poll.receipts().is_empty(), "no try before 40 seconds");
    poll.advance(1);
    poll.folders.scan();
    assert_eq!(poll.receipts().len(), 1);
}

// §6 "Present at selection": a file recorded when the folder was chosen is skipped
// until its size or time changes.
#[test]
fn a_file_present_at_selection_is_skipped_until_it_changes() {
    let poll = Poll::new();
    poll.put("old.csv", EXPORT);
    let candidate = candidates(&poll.folder).unwrap().remove(0);
    let (instrument, path) = (
        poll.instrument.clone(),
        poll.folder.to_str().unwrap().to_owned(),
    );
    Poll::configure(&poll.db, move |ctx| {
        lis_interfaces::set_result_folder(
            ctx,
            SetResultFolder {
                instrument_id: instrument.id,
                expected_revision: instrument.revision,
                folder: Some(WatchedFolder {
                    path,
                    present: vec![PresentFile {
                        file_name: Phi::new(candidate.name),
                        byte_len: candidate.byte_len,
                        modified_at: candidate.modified_at,
                    }],
                }),
            },
        )
    });
    poll.scan_twice();
    poll.advance(10);
    poll.folders.scan();
    assert!(poll.receipts().is_empty());
    let mut changed = EXPORT.to_vec();
    changed.extend_from_slice(b"C00000124,Morphine,12.0\n");
    poll.put("old.csv", &changed);
    poll.scan_twice();
    assert_eq!(poll.receipts().len(), 1);
}

#[test]
fn modification_times_are_utc_instants() {
    let time = UNIX_EPOCH + Duration::from_millis(1_500);
    assert_eq!(
        utc(time).unwrap().to_string(),
        "1970-01-01T00:00:01.500000000Z"
    );
    assert_eq!(utc(UNIX_EPOCH - Duration::from_secs(1)), None);
}

// F13: the poller scans as soon as it starts, then on its interval, and stops on request.
#[test]
fn the_poller_scans_at_start_and_stops() {
    let poll = Poll::new();
    let folders = Arc::new(ResultFolders::new(
        Arc::clone(&poll.db),
        poll.clock.clone(),
        Arc::clone(&poll.listeners),
    ));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let poller = FolderPoller::start(Arc::clone(&folders), Duration::from_millis(10));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while poll.listeners.status_snapshot().is_empty() {
            assert!(std::time::Instant::now() < deadline, "no scan at start");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        poller.stop(&folders).await;
    });
    assert_eq!(poll.status().state, ListenerState::Listening);
}
