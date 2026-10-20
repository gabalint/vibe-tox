//! Result-file receipts, watched folders and migration 0002 (REQ-INT-009,
//! REQ-OPS-013; result-files brief F1, F13, F14, F16). Synthetic data only.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lis_core::{
    Capability, CorrelationId, FixedClock, InstrumentId, Migration, ModuleSchema, Phi, Principal,
};
use lis_interfaces::{
    AbortCode, ConnectionSettings, FileFacts, FileOrigin, FileSource, Instrument,
    InstrumentTransport, InterfaceError, ListenerDetailCode, ListenerState, ListenerSupervisor,
    MAX_RESULT_FILE_BYTES, PresentFile, Receipt, ReceiptId, ReceiptState, ReceiveResultFileInput,
    RegisterInstrument, SCHEMA, SetResultFolder, StatusReport, UpdateInstrumentConnection,
    WatchedFolder,
};
use lis_store::testing::{TestDatabase, principal, test_start};
use lis_store::{DATABASE_FILE, Database, StoreError};

const EXPORT: &[u8] = b"Sample Name,Component,Final Conc\nC00000123,Morphine,152.4\n";

fn admin() -> Principal {
    principal(&[Capability::ManageConfiguration])
}

fn analyst() -> Principal {
    principal(&[Capability::PerformAnalysis])
}

fn write<T: Send + 'static>(
    db: &Database,
    principal: Principal,
    command: impl FnOnce(&mut lis_store::CommandContext<'_>) -> Result<T, InterfaceError>
    + Send
    + 'static,
) -> Result<T, InterfaceError> {
    db.write_blocking(principal, CorrelationId::generate(), command)
}

fn register(
    db: &Database,
    code: &str,
    connection: ConnectionSettings,
    enabled: bool,
) -> Instrument {
    let input = RegisterInstrument {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        connection,
        enabled,
    };
    write(db, admin(), move |ctx| {
        lis_interfaces::register_instrument(ctx, input)
    })
    .unwrap()
}

fn set_folder(
    db: &Database,
    instrument: &Instrument,
    folder: Option<&str>,
) -> Result<Instrument, InterfaceError> {
    choose_folder(db, instrument, folder, Vec::new())
}

/// Sets `folder` with the files `present` in it when it was chosen.
fn choose_folder(
    db: &Database,
    instrument: &Instrument,
    folder: Option<&str>,
    present: Vec<PresentFile>,
) -> Result<Instrument, InterfaceError> {
    let input = SetResultFolder {
        instrument_id: instrument.id,
        expected_revision: instrument.revision,
        folder: folder.map(|path| WatchedFolder {
            path: path.to_owned(),
            present,
        }),
    };
    write(db, admin(), move |ctx| {
        lis_interfaces::set_result_folder(ctx, input)
    })
}

/// A pickup from the instrument's watched folder.
fn folder_of(instrument: &Instrument) -> FileSource {
    FileSource::Folder(instrument.result_folder.clone().unwrap_or_default())
}

/// An absolute synthetic folder on this platform; nothing is created there.
fn synthetic(name: &str) -> String {
    let root = if cfg!(windows) {
        r"C:\synthetic"
    } else {
        "/synthetic"
    };
    format!("{root}{}{name}", std::path::MAIN_SEPARATOR)
}

/// An enabled result-file instrument watching the synthetic folder named `code`.
fn watched(db: &Database, code: &str) -> Instrument {
    let instrument = register(db, code, ConnectionSettings::ResultFile {}, true);
    set_folder(db, &instrument, Some(&synthetic(code))).unwrap()
}

fn tcp_settings(port: u16) -> ConnectionSettings {
    ConnectionSettings::AstmTcp {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
    }
}

fn facts(instrument_id: InstrumentId, source: FileSource, name: &str, byte_len: u64) -> FileFacts {
    FileFacts {
        instrument_id,
        source,
        file_name: Phi::new(name.to_owned()),
        byte_len,
        modified_at: test_start(),
    }
}

/// `bytes` stored as an artifact, offered as the file `name`.
fn stored(db: &Database, file: FileFacts, bytes: &[u8]) -> ReceiveResultFileInput {
    let digest = db.artifacts().put(bytes).unwrap().digest.to_string();
    ReceiveResultFileInput {
        file: FileFacts {
            byte_len: bytes.len() as u64,
            ..file
        },
        sha256: digest.clone(),
        artifact_digest: digest,
    }
}

fn receive(
    db: &Database,
    principal: Principal,
    input: ReceiveResultFileInput,
) -> Result<Receipt, InterfaceError> {
    write(db, principal, move |ctx| {
        lis_interfaces::receive_result_file(ctx, input)
    })
}

fn pick_up(db: &Database, instrument: &Instrument, name: &str, bytes: &[u8]) -> Receipt {
    let input = stored(
        db,
        facts(instrument.id, folder_of(instrument), name, 0),
        bytes,
    );
    receive(db, Principal::instrument(instrument.id), input).unwrap()
}

fn count(db: &Database, sql: &'static str) -> i64 {
    db.read_blocking(move |conn| {
        Ok::<_, InterfaceError>(conn.query_row(sql, [], |row| row.get(0))?)
    })
    .unwrap()
}

fn import_items(db: &Database) -> i64 {
    count(
        db,
        "SELECT COUNT(*) FROM store_outbox WHERE topic = 'ifc.receipt.completed'",
    )
}

fn denied(result: Result<Receipt, InterfaceError>) -> bool {
    matches!(result, Err(InterfaceError::Store(StoreError::Denied(_))))
}

// F1, F16: a folder pickup is a complete receipt of the instrument, durable
// before its import is queued, and the ledger remembers it.
#[test]
fn folder_pickup_is_a_complete_receipt_of_the_instrument_with_an_import() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let receipt = pick_up(&t.db, &instrument, "batch-001.csv", EXPORT);

    assert_eq!(receipt.state, ReceiptState::Complete);
    assert_eq!(
        (
            receipt.frame_count,
            receipt.peer.clone(),
            receipt.duplicate_of
        ),
        (0, None, None)
    );
    assert_eq!(receipt.message_sha256, receipt.artifact_digest);
    let id = receipt.id;
    let file =
        t.db.read_blocking(move |conn| lis_interfaces::receipt_file(conn, id))
            .unwrap()
            .unwrap();
    assert_eq!(
        (file.origin, file.file_name.expose().as_str(), file.byte_len),
        (FileOrigin::Folder, "batch-001.csv", EXPORT.len() as u64)
    );
    assert_eq!(file.loaded_by, None);
    let seen =
        t.db.read_blocking(move |conn| {
            lis_interfaces::seen_file(
                conn,
                instrument.id,
                "batch-001.csv",
                EXPORT.len() as u64,
                test_start(),
            )
        })
        .unwrap();
    assert_eq!(seen, Some(receipt.id));
    assert_eq!(import_items(&t.db), 1);
    assert_eq!(
        count(
            &t.db,
            "SELECT COUNT(*) FROM audit_events WHERE action = 'interface.receive_result_file' \
             AND principal_kind = 'instrument' AND details NOT LIKE '%batch-001%'"
        ),
        1
    );
}

// F16: a pickup needs the instrument's own principal.
#[test]
fn folder_pickup_needs_the_matching_instrument_principal() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let other = watched(&t.db, "LCMS-2");
    let offer = |db: &Database| {
        stored(
            db,
            facts(instrument.id, folder_of(&instrument), "a.csv", 0),
            EXPORT,
        )
    };
    assert!(denied(receive(
        &t.db,
        Principal::instrument(other.id),
        offer(&t.db)
    )));
    let intake_user = principal(&[Capability::InstrumentIntake]);
    assert!(denied(receive(&t.db, intake_user, offer(&t.db))));
    assert!(denied(receive(&t.db, analyst(), offer(&t.db))));
    assert_eq!(count(&t.db, "SELECT COUNT(*) FROM ifc_receipts"), 0);
}

// F16: a manual load needs `perform_analysis` and records who loaded the file.
#[test]
fn manual_load_needs_perform_analysis_and_records_the_user() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = register(&t.db, "LCMS-1", ConnectionSettings::ResultFile {}, true);
    let offer = |db: &Database| {
        stored(
            db,
            facts(instrument.id, FileSource::Upload, "a.csv", 0),
            EXPORT,
        )
    };
    assert!(denied(receive(&t.db, principal(&[]), offer(&t.db))));
    assert!(denied(receive(
        &t.db,
        Principal::instrument(instrument.id),
        offer(&t.db)
    )));
    let system = Principal::system("synthetic-worker", [Capability::PerformAnalysis]);
    assert!(denied(receive(&t.db, system, offer(&t.db))));

    let user = analyst();
    let lis_core::Actor::User { user_id, .. } = *user.actor() else {
        unreachable!("analyst is a user")
    };
    let receipt = receive(&t.db, user, offer(&t.db)).unwrap();
    let file =
        t.db.read_blocking(move |conn| lis_interfaces::receipt_file(conn, receipt.id))
            .unwrap()
            .unwrap();
    assert_eq!(
        (file.origin, file.loaded_by),
        (FileOrigin::Upload, Some(user_id))
    );
    assert_eq!(import_items(&t.db), 1);
    assert_eq!(
        count(
            &t.db,
            "SELECT COUNT(*) FROM audit_events WHERE action = 'interface.receive_result_file' \
             AND principal_kind = 'user' AND stream = 'clinical'"
        ),
        1
    );
}

// F1: only an enabled result-file instrument receives files; a pickup also
// needs a watched folder, so clearing it stops pickups at once.
#[test]
fn files_need_an_enabled_result_file_instrument() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let disabled = register(&t.db, "LCMS-OFF", ConnectionSettings::ResultFile {}, false);
    let astm = register(&t.db, "ASTM-1", tcp_settings(4001), true);
    let input = stored(
        &t.db,
        facts(disabled.id, FileSource::Upload, "a.csv", 0),
        EXPORT,
    );
    assert!(matches!(
        receive(&t.db, analyst(), input),
        Err(InterfaceError::InstrumentDisabled)
    ));
    let input = stored(
        &t.db,
        facts(astm.id, FileSource::Upload, "a.csv", 0),
        EXPORT,
    );
    assert!(matches!(
        receive(&t.db, analyst(), input),
        Err(InterfaceError::TransportMismatch)
    ));
    let manual = register(&t.db, "LCMS-1", ConnectionSettings::ResultFile {}, true);
    let input = stored(
        &t.db,
        facts(
            manual.id,
            FileSource::Folder(synthetic("LCMS-1")),
            "a.csv",
            0,
        ),
        EXPORT,
    );
    assert!(matches!(
        receive(&t.db, Principal::instrument(manual.id), input),
        Err(InterfaceError::ResultFolderNotSet)
    ));
    assert_eq!(count(&t.db, "SELECT COUNT(*) FROM ifc_receipts"), 0);
}

// F12: identical bytes from the same instrument give a duplicate receipt and no
// second import, whatever the file is called.
#[test]
fn identical_bytes_give_a_duplicate_receipt_without_an_import() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let first = pick_up(&t.db, &instrument, "batch-001.csv", EXPORT);
    let renamed = pick_up(&t.db, &instrument, "batch-001-copy.csv", EXPORT);
    assert_eq!(renamed.duplicate_of, Some(first.id));
    assert_eq!(import_items(&t.db), 1);
    let other = watched(&t.db, "LCMS-2");
    assert_eq!(
        pick_up(&t.db, &other, "batch-001.csv", EXPORT).duplicate_of,
        None
    );
}

// F13: the ledger refuses a second pickup of the same file; loading it by hand
// again is allowed and gives a duplicate receipt.
#[test]
fn a_file_already_picked_up_is_refused_and_a_manual_reload_is_a_duplicate() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let first = pick_up(&t.db, &instrument, "batch-001.csv", EXPORT);
    let again = stored(
        &t.db,
        facts(instrument.id, folder_of(&instrument), "batch-001.csv", 0),
        EXPORT,
    );
    assert!(matches!(
        receive(&t.db, Principal::instrument(instrument.id), again),
        Err(InterfaceError::FileAlreadyReceived)
    ));
    let reload = stored(
        &t.db,
        facts(instrument.id, FileSource::Upload, "batch-001.csv", 0),
        EXPORT,
    );
    let reloaded = receive(&t.db, analyst(), reload).unwrap();
    assert_eq!(reloaded.duplicate_of, Some(first.id));
    assert_eq!(count(&t.db, "SELECT COUNT(*) FROM ifc_seen_files"), 1);
    // The same name with new content is a new file (a re-export).
    let changed = pick_up(
        &t.db,
        &instrument,
        "batch-001.csv",
        b"Sample Name\nC00000124\n",
    );
    assert_eq!(changed.duplicate_of, None);
    assert_eq!(import_items(&t.db), 2);
}

// F14: a file over the limit is an aborted receipt with no artifact and no
// import, remembered so it is not recorded again.
#[test]
fn an_oversize_file_is_an_aborted_receipt_without_artifact() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let big = facts(
        instrument.id,
        folder_of(&instrument),
        "big.csv",
        MAX_RESULT_FILE_BYTES + 1,
    );
    let oversize = big.clone();
    let receipt = write(&t.db, Principal::instrument(instrument.id), move |ctx| {
        lis_interfaces::record_oversize_file(ctx, oversize)
    })
    .unwrap();
    assert_eq!(
        (receipt.state, receipt.abort_reason, receipt.artifact_digest),
        (ReceiptState::Aborted, Some(AbortCode::FileTooLarge), None)
    );
    assert_eq!(import_items(&t.db), 0);
    assert!(matches!(
        write(&t.db, Principal::instrument(instrument.id), move |ctx| {
            lis_interfaces::record_oversize_file(ctx, big)
        }),
        Err(InterfaceError::FileAlreadyReceived)
    ));
    let small = facts(
        instrument.id,
        FileSource::Upload,
        "small.csv",
        MAX_RESULT_FILE_BYTES,
    );
    assert!(matches!(
        write(&t.db, analyst(), move |ctx| {
            lis_interfaces::record_oversize_file(ctx, small)
        }),
        Err(InterfaceError::InvalidFileSize)
    ));
    let mut too_big = stored(
        &t.db,
        facts(instrument.id, FileSource::Upload, "x.csv", 0),
        EXPORT,
    );
    too_big.file.byte_len = MAX_RESULT_FILE_BYTES + 1;
    assert!(matches!(
        receive(&t.db, analyst(), too_big),
        Err(InterfaceError::InvalidFileSize)
    ));
}

#[test]
fn a_digest_that_is_not_the_artifact_is_refused() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let mut input = stored(
        &t.db,
        facts(instrument.id, FileSource::Upload, "a.csv", 0),
        EXPORT,
    );
    input.sha256 = "0".repeat(64);
    assert!(matches!(
        receive(&t.db, analyst(), input),
        Err(InterfaceError::ArtifactMismatch)
    ));
}

// F13: a folder belongs to one instrument; setting it needs
// `manage_configuration`, a result-file instrument, an absolute path and the
// current revision, and is audited.
#[test]
fn a_watched_folder_belongs_to_one_result_file_instrument() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let first = watched(&t.db, "LCMS-1");
    assert_eq!(
        first.result_folder.as_deref(),
        Some(synthetic("LCMS-1").as_str())
    );
    let second = register(&t.db, "LCMS-2", ConnectionSettings::ResultFile {}, true);
    assert!(matches!(
        set_folder(&t.db, &second, Some(synthetic("LCMS-1").as_str())),
        Err(InterfaceError::ResultFolderInUse)
    ));
    assert!(matches!(
        set_folder(&t.db, &second, Some("relative/folder")),
        Err(InterfaceError::InvalidResultFolder)
    ));
    let astm = register(&t.db, "ASTM-1", tcp_settings(4001), true);
    assert!(matches!(
        set_folder(&t.db, &astm, Some(synthetic("astm").as_str())),
        Err(InterfaceError::TransportMismatch)
    ));
    let stale = Instrument {
        revision: 1,
        ..first.clone()
    };
    assert!(matches!(
        set_folder(&t.db, &stale, None),
        Err(InterfaceError::RevisionConflict)
    ));
    let input = SetResultFolder {
        instrument_id: second.id,
        expected_revision: second.revision,
        folder: Some(WatchedFolder {
            path: synthetic("LCMS-2"),
            present: Vec::new(),
        }),
    };
    let refused = write(&t.db, analyst(), move |ctx| {
        lis_interfaces::set_result_folder(ctx, input)
    });
    assert!(matches!(
        refused,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));

    let cleared = set_folder(&t.db, &first, None).unwrap();
    assert_eq!(cleared.result_folder, None);
    let moved = set_folder(&t.db, &second, Some(synthetic("LCMS-1").as_str())).unwrap();
    assert_eq!(moved.revision, second.revision + 1);
    assert_eq!(
        count(
            &t.db,
            "SELECT COUNT(*) FROM audit_events WHERE action = 'interface.set_result_folder' \
             AND stream = 'security' AND outcome = 'succeeded'"
        ),
        3
    );
}

// The folder belongs to the result-file connection: another kind drops it.
#[test]
fn changing_the_connection_kind_drops_the_folder() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let input = UpdateInstrumentConnection {
        instrument_id: instrument.id,
        expected_revision: instrument.revision,
        connection: tcp_settings(4002),
    };
    let changed = write(&t.db, admin(), move |ctx| {
        lis_interfaces::update_instrument_connection(ctx, input)
    })
    .unwrap();
    assert_eq!(changed.result_folder, None);
    let id = instrument.id;
    let stored =
        t.db.read_blocking(move |conn| lis_interfaces::get_instrument(conn, &admin(), id))
            .unwrap();
    assert_eq!(stored, changed);
}

fn wait_for(supervisor: &ListenerSupervisor, id: InstrumentId, state: ListenerState) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !supervisor
        .status_snapshot()
        .iter()
        .any(|status| status.instrument_id == id && status.state == state)
    {
        assert!(Instant::now() < deadline, "no {state:?} status");
        std::thread::sleep(Duration::from_millis(5));
    }
}

// F13, REQ-OPS-013: result-file instruments get no listener thread; the folder
// poller reports their state, but never over a running listener's.
#[test]
fn folder_status_never_replaces_a_running_listener() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(FixedClock::new(test_start()));
    let db = Arc::new(Database::open(dir.path(), &[SCHEMA], clock).unwrap());
    let files = watched(&db, "LCMS-1");
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let astm = register(&db, "ASTM-1", tcp_settings(port), true);
    let supervisor = ListenerSupervisor::new(db);
    supervisor.start_all().unwrap();
    wait_for(&supervisor, astm.id, ListenerState::Listening);
    assert!(
        supervisor
            .status_snapshot()
            .iter()
            .all(|status| status.instrument_id != files.id),
        "no listener runs for a result-file instrument"
    );
    supervisor.report_folder_status(
        files.id,
        StatusReport {
            state: ListenerState::Listening,
            detail_code: Some(ListenerDetailCode::FilesWaiting),
            files_waiting: Some(2),
            files_too_large: Some(0),
        },
    );
    let error = StatusReport {
        state: ListenerState::Error,
        detail_code: None,
        files_waiting: None,
        files_too_large: None,
    };
    supervisor.report_folder_status(astm.id, error);
    let snapshot = supervisor.status_snapshot();
    let of = |id| {
        snapshot
            .iter()
            .find(|status| status.instrument_id == id)
            .unwrap()
    };
    assert_eq!(of(astm.id).state, ListenerState::Listening);
    assert!(!of(astm.id).in_error());
    assert_eq!(of(files.id).files_waiting, Some(2));
    // Files waiting are a counted warning for health, not a folder in error.
    assert!(!of(files.id).in_error());
    supervisor.stop_all().unwrap();
}

fn present(name: &str, byte_len: u64) -> PresentFile {
    PresentFile {
        file_name: Phi::new(name.to_owned()),
        byte_len,
        modified_at: test_start(),
    }
}

/// Whether the poller would skip `name` of `byte_len` bytes in `folder` as present at
/// selection.
fn at_selection(
    db: &Database,
    instrument_id: InstrumentId,
    folder: &str,
    name: &str,
    byte_len: u64,
) -> bool {
    let (folder, name) = (folder.to_owned(), name.to_owned());
    db.read_blocking(move |conn| {
        lis_interfaces::present_at_selection(
            conn,
            instrument_id,
            &folder,
            &name,
            byte_len,
            test_start(),
        )
    })
    .unwrap()
}

// §6 "Present at selection": the files already in a folder when it is chosen are
// recorded and never picked up; a changed file is a new file; a manual load is still
// allowed; the baseline is append-only.
#[test]
fn files_present_when_a_folder_is_chosen_are_not_picked_up() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let registered = register(&t.db, "LCMS-1", ConnectionSettings::ResultFile {}, true);
    let len = EXPORT.len() as u64;
    let instrument = choose_folder(
        &t.db,
        &registered,
        Some(synthetic("LCMS-1").as_str()),
        vec![present("old-1.csv", len), present("old-2.csv", len)],
    )
    .unwrap();
    assert!(at_selection(
        &t.db,
        instrument.id,
        &synthetic("LCMS-1"),
        "old-1.csv",
        len
    ));
    assert!(!at_selection(
        &t.db,
        instrument.id,
        &synthetic("LCMS-1"),
        "old-1.csv",
        len + 1
    ));
    assert!(!at_selection(
        &t.db,
        instrument.id,
        &synthetic("other"),
        "old-1.csv",
        len
    ));
    let pickup = stored(
        &t.db,
        facts(instrument.id, folder_of(&instrument), "old-1.csv", 0),
        EXPORT,
    );
    assert!(matches!(
        receive(&t.db, Principal::instrument(instrument.id), pickup),
        Err(InterfaceError::FilePresentAtSelection)
    ));
    assert_eq!(count(&t.db, "SELECT COUNT(*) FROM ifc_receipts"), 0);
    // The same name with another size is no longer the file that was there.
    let rewritten = pick_up(&t.db, &instrument, "old-2.csv", b"Sample Name\nC00000124\n");
    assert_eq!(rewritten.state, ReceiptState::Complete);
    let by_hand = stored(
        &t.db,
        facts(instrument.id, FileSource::Upload, "old-1.csv", 0),
        EXPORT,
    );
    assert!(receive(&t.db, analyst(), by_hand).is_ok());
    assert_eq!(
        count(
            &t.db,
            "SELECT COUNT(*) FROM audit_events WHERE action = 'interface.set_result_folder' \
             AND details LIKE '%\"files_present\"%'"
        ),
        1
    );
    // Choosing the same folder again keeps the recorded files.
    let again = choose_folder(
        &t.db,
        &instrument,
        Some(synthetic("LCMS-1").as_str()),
        vec![present("old-1.csv", len), present("old-3.csv", len)],
    )
    .unwrap();
    assert_eq!(again.revision, instrument.revision + 1);
    assert_eq!(
        count(&t.db, "SELECT COUNT(*) FROM ifc_folder_baseline_files"),
        3
    );
    t.db.write_blocking(admin(), CorrelationId::generate(), |ctx| {
        let refused = |sql: &str| ctx.tx().execute(sql, []).is_err();
        assert!(refused("UPDATE ifc_folder_baseline_files SET byte_len = 0"));
        assert!(refused("DELETE FROM ifc_folder_baseline_files"));
        Ok::<_, InterfaceError>(())
    })
    .unwrap();
}

// §6: a folder moved to another instrument is not imported again for it, because its
// files are recorded as present when it is chosen there.
#[test]
fn a_folder_moved_to_another_instrument_is_not_imported_again() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let first = watched(&t.db, "LCMS-1");
    pick_up(&t.db, &first, "batch-001.csv", EXPORT);
    set_folder(&t.db, &first, None).unwrap();
    let second = register(&t.db, "LCMS-2", ConnectionSettings::ResultFile {}, true);
    let len = EXPORT.len() as u64;
    let moved = choose_folder(
        &t.db,
        &second,
        Some(synthetic("LCMS-1").as_str()),
        vec![present("batch-001.csv", len)],
    )
    .unwrap();
    let again = stored(
        &t.db,
        facts(moved.id, folder_of(&moved), "batch-001.csv", 0),
        EXPORT,
    );
    assert!(matches!(
        receive(&t.db, Principal::instrument(moved.id), again),
        Err(InterfaceError::FilePresentAtSelection)
    ));
    assert_eq!(count(&t.db, "SELECT COUNT(*) FROM ifc_receipts"), 1);
}

// L11 of the wave-10 review: a pickup names the folder it was listed in; the receipt is
// refused once the instrument watches another folder or none.
#[test]
fn a_pickup_from_a_folder_no_longer_watched_is_refused() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let instrument = watched(&t.db, "LCMS-1");
    let listed = folder_of(&instrument);
    let moved = set_folder(&t.db, &instrument, Some(synthetic("elsewhere").as_str())).unwrap();
    let offer = |db: &Database| stored(db, facts(moved.id, listed.clone(), "a.csv", 0), EXPORT);
    assert!(matches!(
        receive(&t.db, Principal::instrument(moved.id), offer(&t.db)),
        Err(InterfaceError::ResultFolderChanged)
    ));
    set_folder(&t.db, &moved, None).unwrap();
    assert!(matches!(
        receive(&t.db, Principal::instrument(moved.id), offer(&t.db)),
        Err(InterfaceError::ResultFolderNotSet)
    ));
    assert_eq!(count(&t.db, "SELECT COUNT(*) FROM ifc_receipts"), 0);
}

// L5 of the wave-10 review: intake reads whether the instrument can receive a file
// before it stores any bytes.
#[test]
fn only_an_enabled_result_file_instrument_can_receive_files() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let files = register(&t.db, "LCMS-1", ConnectionSettings::ResultFile {}, true);
    let disabled = register(&t.db, "LCMS-OFF", ConnectionSettings::ResultFile {}, false);
    let astm = register(&t.db, "ASTM-1", tcp_settings(4003), true);
    let receivable = |id: InstrumentId| {
        t.db.read_blocking(move |conn| lis_interfaces::receivable_instrument(conn, id))
    };
    assert_eq!(receivable(files.id).unwrap().id, files.id);
    assert!(matches!(
        receivable(disabled.id),
        Err(InterfaceError::InstrumentDisabled)
    ));
    assert!(matches!(
        receivable(astm.id),
        Err(InterfaceError::TransportMismatch)
    ));
    assert!(matches!(
        receivable(InstrumentId::generate()),
        Err(InterfaceError::InstrumentNotFound)
    ));
}

/// Migration 0001 alone, as databases created before result files have it.
const BEFORE_FILES: ModuleSchema = ModuleSchema {
    module: SCHEMA.module,
    table_prefix: SCHEMA.table_prefix,
    migrations: &[Migration {
        version: 1,
        name: "instruments_and_receipts",
        sql: include_str!("../migrations/0001_instruments_and_receipts.sql"),
    }],
};

const TCP_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8057";
const SERIAL_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8058";
const COMPLETE_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8061";
const DUPLICATE_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8062";
const RECEIVING_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8063";
const DIGEST: &str = "8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4";

fn seed_before_files(path: &std::path::Path) {
    drop(
        Database::open(
            path,
            &[BEFORE_FILES],
            Arc::new(FixedClock::new(test_start())),
        )
        .unwrap(),
    );
    let conn = rusqlite::Connection::open(path.join(DATABASE_FILE)).unwrap();
    let at = test_start().to_string();
    conn.execute_batch(&format!(
        "INSERT INTO ifc_instruments (id, code, name, connection_kind, tcp_bind_address, tcp_port,
             tcp_allowed_peers, enabled, revision)
         VALUES ('{TCP_ID}', 'ASTM-TCP', 'Synthetic TCP', 'astm_tcp', '127.0.0.1', 4100,
             '[\"127.0.0.1\"]', 1, 3);
         INSERT INTO ifc_instruments (id, code, name, connection_kind, serial_port_path,
             serial_baud_rate, serial_data_bits, serial_parity, serial_stop_bits, enabled, revision)
         VALUES ('{SERIAL_ID}', 'ASTM-SER', 'Synthetic serial', 'astm_serial', '/dev/synthetic',
             9600, 8, 'none', 1, 0, 1);
         INSERT INTO ifc_receipts (id, instrument_id, transport, peer, started_at, completed_at,
             state, frame_count, message_sha256, artifact_digest)
         VALUES ('{COMPLETE_ID}', '{TCP_ID}', 'astm_tcp', '127.0.0.1:50000', '{at}', '{at}',
             'complete', 1, '{DIGEST}', '{DIGEST}');
         INSERT INTO ifc_receipts (id, instrument_id, transport, peer, started_at, completed_at,
             state, frame_count, message_sha256, artifact_digest, duplicate_of)
         VALUES ('{DUPLICATE_ID}', '{TCP_ID}', 'astm_tcp', '127.0.0.1:50001', '{at}', '{at}',
             'complete', 1, '{DIGEST}', '{DIGEST}', '{COMPLETE_ID}');
         INSERT INTO ifc_receipts (id, instrument_id, transport, started_at, state, frame_count)
         VALUES ('{RECEIVING_ID}', '{SERIAL_ID}', 'astm_serial', '{at}', 'receiving', 1);
         INSERT INTO ifc_receipt_frames (receipt_id, ordinal, bytes)
         VALUES ('{RECEIVING_ID}', 0, X'0231487C0D03');"
    ))
    .unwrap();
}

/// Index and trigger names on the interface tables.
fn schema_objects(conn: &rusqlite::Connection) -> Vec<String> {
    conn.prepare(
        "SELECT type || ' ' || name FROM sqlite_schema
         WHERE type IN ('index', 'trigger') AND tbl_name LIKE 'ifc_%' ORDER BY type, name",
    )
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// A database seeded before result files, migrated by opening it with migration 0002;
/// also the index and trigger names it had before.
fn migrated() -> (tempfile::TempDir, Database, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    seed_before_files(dir.path());
    let before =
        schema_objects(&rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap());
    let db = Database::open(
        dir.path(),
        &[SCHEMA],
        Arc::new(FixedClock::new(test_start())),
    )
    .unwrap();
    (dir, db, before)
}

// Migration 0002 rebuilds the instrument and receipt tables: every ASTM instrument,
// receipt and frame survives.
#[test]
fn migration_keeps_astm_instruments_and_receipts() {
    let (_dir, db, _) = migrated();
    let instruments = db
        .read_blocking(|conn| lis_interfaces::list_instruments(conn, &admin()))
        .unwrap();
    let summary: Vec<_> = instruments
        .iter()
        .map(|i| {
            (
                i.code.as_str(),
                i.connection.transport(),
                i.enabled,
                i.revision,
                i.result_folder.clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("ASTM-SER", InstrumentTransport::AstmSerial, false, 1, None),
            ("ASTM-TCP", InstrumentTransport::AstmTcp, true, 3, None),
        ]
    );
    let reader = Principal::system("synthetic-reader", [Capability::InstrumentIntake]);
    let receipt = |id: &str| {
        let id: ReceiptId = id.parse().unwrap();
        let reader = reader.clone();
        db.read_blocking(move |conn| lis_interfaces::get_receipt(conn, &reader, id))
            .unwrap()
    };
    assert_eq!(
        receipt(DUPLICATE_ID).duplicate_of,
        Some(COMPLETE_ID.parse().unwrap())
    );
    assert_eq!(
        receipt(COMPLETE_ID).artifact_digest.as_deref(),
        Some(DIGEST)
    );
    assert_eq!(receipt(RECEIVING_ID).state, ReceiptState::Receiving);
    let frames = db
        .read_blocking(move |conn| {
            lis_interfaces::list_receipt_frames(conn, &reader, RECEIVING_ID.parse().unwrap())
        })
        .unwrap();
    assert_eq!(frames.len(), 1);
}

// The rebuilt tables keep their indexes, triggers and foreign keys, and the new file
// tables are append-only.
#[test]
fn migration_keeps_the_receipt_guards() {
    let (dir, db, before) = migrated();
    drop(db);
    let conn = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let after = schema_objects(&conn);
    for object in &before {
        assert!(after.contains(object), "{object} lost");
    }
    let new: Vec<_> = after.iter().filter(|o| !before.contains(o)).collect();
    assert_eq!(
        new,
        [
            "index ifc_instruments_result_folder",
            "index sqlite_autoindex_ifc_au_multiblock_guards_1",
            "index sqlite_autoindex_ifc_folder_baseline_files_1",
            "index sqlite_autoindex_ifc_receipt_au_profiles_1",
            "index sqlite_autoindex_ifc_receipt_files_1",
            "index sqlite_autoindex_ifc_seen_files_1",
            "index sqlite_autoindex_ifc_seen_files_2",
            "trigger ifc_au_multiblock_guard_no_delete",
            "trigger ifc_au_multiblock_guard_no_update",
            "trigger ifc_au_multiblock_guard_receiving_only",
            "trigger ifc_folder_baseline_file_no_delete",
            "trigger ifc_folder_baseline_file_no_update",
            "trigger ifc_receipt_au_profile_no_delete",
            "trigger ifc_receipt_au_profile_no_update",
            "trigger ifc_receipt_au_profile_receiving_only",
            "trigger ifc_receipt_file_no_delete",
            "trigger ifc_receipt_file_no_update",
            "trigger ifc_receipt_file_result_file_only",
            "trigger ifc_seen_file_no_delete",
            "trigger ifc_seen_file_no_update",
        ]
    );
    let refused = |sql: &str| conn.execute(sql, []).is_err();
    assert!(refused(&format!(
        "UPDATE ifc_receipts SET peer = 'x' WHERE id = '{COMPLETE_ID}'"
    )));
    assert!(refused(&format!(
        "DELETE FROM ifc_receipts WHERE id = '{COMPLETE_ID}'"
    )));
    assert!(refused(&format!(
        "INSERT INTO ifc_receipt_frames (receipt_id, ordinal, bytes) VALUES ('{COMPLETE_ID}', 1, X'00')"
    )));
    assert!(refused(&format!(
        "INSERT INTO ifc_receipt_files (receipt_id, origin, file_name, byte_len, modified_at)
         VALUES ('{COMPLETE_ID}', 'folder', 'a.csv', 1, '{}')",
        test_start()
    )));
    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}
