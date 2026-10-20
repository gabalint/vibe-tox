//! Result-file intake through the engine (REQ-INT-009, REQ-OPS-013; result-files brief
//! §6, F1, F12, F13, F14, F16): the watched folder, picked up at the sign-in screen and
//! after a restart, and loading a file by hand. The engine clock is fixed, so the
//! poller's stability rule is driven by moving it. Synthetic files only.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lis_auth::{DataKey, RoleCode};
use lis_core::{CommandError, ErrorCode, FixedClock, InstrumentId, ModuleSchema, UserId};
use lis_engine::{
    ChosenResultFolder, DialogCommand, Engine, EngineConfig, Guard, HealthState, Inhibitor,
    InstallationKind, Instrument, KeyProvider, LoadOutcome, LoadResultFileInput,
    PrinterDestinations, RegisterInstrument, ResultFolderInput, SetInstrumentEnabled,
};
use lis_interfaces::{ConnectionSettings, ListenerState, MAX_RESULT_FILE_BYTES};
use lis_ops::health::CheckCode;
use lis_store::{DATABASE_FILE, Database};
use tempfile::TempDir;

use crate::fixture::{TestEngine, code_of, start_time};

const EXPORT: &[u8] = b"Sample Name,Component,Final Conc\nC00000123,Morphine,152.4\n";
const REEXPORT: &[u8] =
    b"Sample Name,Component,Final Conc\nC00000123,Morphine,148.9\nC00000124,Morphine,12.0\n";

/// Calls the result-file command `name` with plausible input and a cancelled dialog;
/// `None` for other areas.
pub(crate) async fn result_files_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    let instrument_id = InstrumentId::generate();
    let folder = ResultFolderInput {
        instrument_id,
        expected_revision: 1,
    };
    Some(match name {
        "load_result_file" => engine
            .load_result_file(LoadResultFileInput { instrument_id }, None)
            .await
            .map(drop),
        "choose_result_folder" => engine.choose_result_folder(folder, None).await.map(drop),
        "clear_result_folder" => engine.clear_result_folder(folder).await.map(drop),
        _ => return None,
    })
}

/// A signed-in administrator's engine with an enabled result-file instrument watching an
/// empty temporary folder.
struct Watched {
    t: TestEngine,
    instrument: Instrument,
    folder: TempDir,
}

impl Watched {
    async fn new() -> Self {
        let t = TestEngine::start().await;
        t.admin_session().await;
        let registered = t
            .engine
            .register_instrument(RegisterInstrument {
                code: "LCMS-1".to_owned(),
                name: "Synthetic LC-MS/MS".to_owned(),
                connection: ConnectionSettings::ResultFile {},
                enabled: true,
            })
            .await
            .unwrap()
            .instrument;
        let folder = tempfile::tempdir().unwrap();
        let chosen = choose(&t.engine, &registered, folder.path())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(chosen.present_files, 0, "an empty folder");
        let instrument = chosen.instrument;
        let canonical = fs::canonicalize(folder.path()).unwrap();
        assert_eq!(
            instrument.result_folder.as_deref(),
            canonical.to_str(),
            "the canonical folder is stored"
        );
        Self {
            t,
            instrument,
            folder,
        }
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        fs::write(self.folder.path().join(name), bytes).unwrap();
    }

    /// Two scans 10 seconds apart by the engine clock: a stable file is then received.
    async fn pick_up(&self) {
        self.t.engine.scan_result_folders().await;
        self.t.advance(10);
        self.t.engine.scan_result_folders().await;
    }

    /// `(origin, duplicate, state)` of every file receipt, oldest first.
    fn receipts(&self) -> Vec<(String, bool, String)> {
        self.t
            .conn()
            .prepare(
                "SELECT f.origin, r.duplicate_of IS NOT NULL, r.state FROM ifc_receipts r
                 JOIN ifc_receipt_files f ON f.receipt_id = r.id ORDER BY r.rowid",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// Signs the administrator out and an analyst in.
    async fn analyst_session(&self) {
        self.t.create_user("analyst1", &[RoleCode::Analyst]).await;
        self.t.engine.sign_out().await.unwrap();
        self.t.first_sign_in("analyst1").await;
    }

    async fn load(
        &self,
        path: &Path,
    ) -> Result<Option<lis_engine::LoadedResultFile>, CommandError> {
        self.t
            .engine
            .load_result_file(
                LoadResultFileInput {
                    instrument_id: self.instrument.id,
                },
                Some(path.to_owned()),
            )
            .await
    }
}

async fn choose(
    engine: &Engine,
    instrument: &Instrument,
    folder: &Path,
) -> Result<Option<ChosenResultFolder>, CommandError> {
    engine
        .choose_result_folder(
            ResultFolderInput {
                instrument_id: instrument.id,
                expected_revision: instrument.revision,
            },
            Some(folder.to_owned()),
        )
        .await
}

/// Once the import outbox item has finished: its delivery attempts, and the failure
/// code of each import attempt of a file receipt.
async fn imported(t: &TestEngine) -> (i64, Vec<Option<String>>) {
    let conn = t.conn();
    for _ in 0..400 {
        let done: Option<i64> = conn
            .query_row(
                "SELECT attempts FROM store_outbox
                 WHERE topic = 'ifc.receipt.completed' AND state = 'done'",
                [],
                |row| row.get(0),
            )
            .ok();
        if let Some(deliveries) = done {
            let failures = conn
                .prepare(
                    "SELECT a.failure_code FROM res_import_attempts a
                     JOIN ifc_receipt_files f ON f.receipt_id = a.receipt_id",
                )
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            return (deliveries, failures);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the receipt was not imported");
}

// F13, F16, REQ-OPS-013: with nobody signed in, the folder is still watched; the file is
// received as the instrument, and its import runs once, through the outbox.
#[tokio::test]
async fn a_watched_folder_is_picked_up_at_the_sign_in_screen() {
    let w = Watched::new().await;
    w.t.engine.sign_out().await.unwrap();
    w.put("batch-001.csv", EXPORT);
    w.pick_up().await;

    assert_eq!(
        w.receipts(),
        [("folder".to_owned(), false, "complete".to_owned())]
    );
    let principal: String =
        w.t.conn()
            .query_row(
                "SELECT principal_kind || ':' || principal_id FROM audit_events
             WHERE action = 'interface.receive_result_file'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(principal, format!("instrument:{}", w.instrument.id));
    let status = w.t.engine.list_listener_status().await.unwrap();
    assert_eq!(status[0].state, ListenerState::Listening);
    // No file template is registered, so the import fails as a whole
    // (`unregistered_source`; Retry import after registration): one delivery, one
    // terminal attempt, no retry loop.
    let (deliveries, failures) = imported(&w.t).await;
    assert_eq!(deliveries, 1);
    assert_eq!(failures, [Some("unregistered_source".to_owned())]);
}

// F13: a file written while the app was closed is read after the next start.
#[tokio::test]
async fn a_file_written_while_the_app_was_closed_is_read_at_the_next_start() {
    let Watched {
        t,
        instrument,
        folder,
    } = Watched::new().await;
    t.engine.shutdown().await;
    fs::write(folder.path().join("overnight.csv"), EXPORT).unwrap();
    let t = t.crash_and_restart().await;
    let w = Watched {
        t,
        instrument,
        folder,
    };
    w.pick_up().await;
    assert_eq!(
        w.receipts(),
        [("folder".to_owned(), false, "complete".to_owned())]
    );
}

// F12, F13: a file is picked up once; loading it again by hand gives a duplicate receipt;
// a new export under the same name is a new file.
#[tokio::test]
async fn the_same_file_is_received_once_and_a_new_export_under_its_name_again() {
    let w = Watched::new().await;
    w.put("batch-001.csv", EXPORT);
    w.pick_up().await;
    w.pick_up().await;
    assert_eq!(w.receipts().len(), 1, "a received file is not read again");

    w.analyst_session().await;
    let reloaded = w
        .load(&w.folder.path().join("batch-001.csv"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.outcome, LoadOutcome::Duplicate);
    assert!(reloaded.duplicate_of.is_some());

    // A re-export copied over the old file: new size, new content.
    w.put("batch-001.csv", REEXPORT);
    w.pick_up().await;
    assert_eq!(
        w.receipts(),
        [
            ("folder".to_owned(), false, "complete".to_owned()),
            ("upload".to_owned(), true, "complete".to_owned()),
            ("folder".to_owned(), false, "complete".to_owned()),
        ]
    );
}

// F16: loading a file needs `perform_analysis`, also when the dialog is cancelled; the
// user is recorded. F14: a file over the limit is an aborted receipt.
#[tokio::test]
async fn loading_a_file_needs_perform_analysis() {
    let w = Watched::new().await;
    let source = tempfile::tempdir().unwrap();
    let export = source.path().join("batch-002.csv");
    fs::write(&export, EXPORT).unwrap();
    assert_eq!(code_of(w.load(&export).await), ErrorCode::Forbidden);
    assert_eq!(
        code_of(
            w.t.engine
                .preflight_dialog(DialogCommand::LoadResultFile)
                .await
        ),
        ErrorCode::Forbidden
    );
    assert!(w.receipts().is_empty());

    w.analyst_session().await;
    w.t.engine
        .preflight_dialog(DialogCommand::LoadResultFile)
        .await
        .unwrap();
    let cancelled =
        w.t.engine
            .load_result_file(
                LoadResultFileInput {
                    instrument_id: w.instrument.id,
                },
                None,
            )
            .await
            .unwrap();
    assert!(cancelled.is_none());
    let loaded = w.load(&export).await.unwrap().unwrap();
    assert_eq!(loaded.outcome, LoadOutcome::Received);
    assert_eq!(loaded.byte_len, EXPORT.len() as u64);
    assert_eq!(loaded.file_name.expose(), "batch-002.csv");
    let loaded_by: String = w
        .t
        .conn()
        .query_row(
            "SELECT u.username FROM ifc_receipt_files f JOIN auth_users u ON u.id = f.loaded_by",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(loaded_by, "analyst1");

    let huge = source.path().join("huge.csv");
    fs::File::create(&huge)
        .unwrap()
        .set_len(MAX_RESULT_FILE_BYTES + 1)
        .unwrap();
    let refused = w.load(&huge).await.unwrap().unwrap();
    assert_eq!(
        (refused.outcome, refused.sha256),
        (LoadOutcome::TooLarge, None)
    );
    assert_eq!(w.receipts()[1].2, "aborted");
}

// L5 of the wave-10 review: a file chosen for an instrument that cannot receive files now
// is refused before any byte is stored.
#[tokio::test]
async fn a_file_for_a_disabled_instrument_is_not_stored() {
    let w = Watched::new().await;
    w.t.engine
        .set_instrument_enabled(SetInstrumentEnabled {
            instrument_id: w.instrument.id,
            expected_revision: w.instrument.revision,
            enabled: false,
        })
        .await
        .unwrap();
    w.analyst_session().await;
    let source = tempfile::tempdir().unwrap();
    let export = source.path().join("batch-009.csv");
    fs::write(&export, EXPORT).unwrap();
    let artifacts = || {
        fs::read_dir(w.t.data_dir().join(lis_store::ARTIFACTS_DIR))
            .unwrap()
            .count()
    };
    let before = artifacts();
    assert_eq!(code_of(w.load(&export).await), ErrorCode::Conflict);
    assert_eq!(artifacts(), before, "nothing stored");
    assert!(w.receipts().is_empty());
}

// F13: a watched folder exists, can be read and belongs to one instrument; clearing it
// stops pickup.
#[tokio::test]
async fn a_watched_folder_must_be_readable_and_belong_to_one_instrument() {
    let w = Watched::new().await;
    let other =
        w.t.engine
            .register_instrument(RegisterInstrument {
                code: "LCMS-2".to_owned(),
                name: "Synthetic LC-MS/MS 2".to_owned(),
                connection: ConnectionSettings::ResultFile {},
                enabled: true,
            })
            .await
            .unwrap()
            .instrument;
    let missing = w.folder.path().join("missing");
    assert_eq!(
        code_of(choose(&w.t.engine, &other, &missing).await),
        ErrorCode::Validation
    );
    w.put("batch-001.csv", EXPORT);
    let file = w.folder.path().join("batch-001.csv");
    assert_eq!(
        code_of(choose(&w.t.engine, &other, &file).await),
        ErrorCode::Validation
    );
    assert_eq!(
        code_of(choose(&w.t.engine, &other, w.folder.path()).await),
        ErrorCode::Conflict
    );
    w.t.engine
        .preflight_dialog(DialogCommand::ChooseResultFolder)
        .await
        .unwrap();

    let cleared =
        w.t.engine
            .clear_result_folder(ResultFolderInput {
                instrument_id: w.instrument.id,
                expected_revision: w.instrument.revision,
            })
            .await
            .unwrap();
    assert_eq!(cleared.result_folder, None);
    w.pick_up().await;
    assert!(w.receipts().is_empty(), "a cleared folder is not watched");
    let moved = choose(&w.t.engine, &other, w.folder.path())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(moved.instrument.result_folder, w.instrument.result_folder);
    // §6 "Present at selection": the file already there is not imported for the
    // instrument the folder moved to.
    assert_eq!(moved.present_files, 1);
    w.pick_up().await;
    assert!(
        w.receipts().is_empty(),
        "files present at selection are skipped"
    );
}

// §6 "Present at selection": pointing an instrument at a folder that already holds
// exports loads none of them; a new export is picked up, and an old one can still be
// loaded by hand.
#[tokio::test]
async fn files_already_in_a_chosen_folder_are_left_for_loading_by_hand() {
    let w = Watched::new().await;
    let cleared =
        w.t.engine
            .clear_result_folder(ResultFolderInput {
                instrument_id: w.instrument.id,
                expected_revision: w.instrument.revision,
            })
            .await
            .unwrap();
    w.put("old-001.csv", EXPORT);
    w.put("old-002.txt", REEXPORT);
    w.put("notes.json", b"{}");
    let chosen = choose(&w.t.engine, &cleared, w.folder.path())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(chosen.present_files, 2, "only result files are counted");
    w.pick_up().await;
    assert!(w.receipts().is_empty());
    w.put(
        "new-003.csv",
        b"Sample Name,Component,Final Conc\nC00000125,Morphine,9.1\n",
    );
    w.pick_up().await;
    assert_eq!(
        w.receipts(),
        [("folder".to_owned(), false, "complete".to_owned())]
    );
    w.analyst_session().await;
    let loaded = w
        .load(&w.folder.path().join("old-001.csv"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.outcome, LoadOutcome::Received);
}

// REQ-OPS-013, REQ-OPS-008: a watched folder counts as a listener: it keeps the computer
// awake, is named in the close warning, and a missing folder is a listener alert. The
// operations loops run on a multi-thread runtime, as in the app.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_watched_folder_counts_as_a_listener_for_sleep_quit_and_health() {
    let w = Watched::new().await;
    w.t.engine.scan_result_folders().await;
    let held = Arc::new(AtomicBool::new(false));
    let acquired = Arc::clone(&held);
    w.t.engine.start_operations(Inhibitor::new(move || {
        acquired.store(true, Ordering::SeqCst);
        Ok(Box::new(()) as Guard)
    }));
    // The light loop's first step writes a heartbeat before it asks for the inhibitor.
    for _ in 0..1000 {
        if held.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(held.load(Ordering::SeqCst), "sleep is prevented");
    // The job runner's catch-up pass may add its own warning.
    let warnings = w.t.engine.quit_warnings().await.0;
    assert!(
        warnings.contains(&"Synthetic LC-MS/MS is listening for results.".to_owned()),
        "{warnings:?}"
    );

    fs::remove_dir(w.folder.path()).unwrap();
    w.t.engine.scan_result_folders().await;
    let health = w.t.engine.get_host_health().await.unwrap();
    let listeners = health
        .checks
        .iter()
        .find(|check| check.check == CheckCode::Listeners)
        .unwrap();
    assert_eq!(listeners.state, HealthState::Alert);
    w.t.engine.shutdown().await;
}

const TCP_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8057";
const RECEIPT_ID: &str = "01890a5d-ac96-774b-bcce-b302099a8061";
const DIGEST: &str = "8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4";

// Migration 0002 on an installation with every other module: the ASTM instrument, its
// receipt and the analytical record that references the instrument survive the rebuild.
#[tokio::test]
async fn an_installation_with_astm_records_upgrades_to_result_files() {
    let dir = tempfile::tempdir().unwrap();
    let interfaces_v1 = ModuleSchema {
        migrations: &lis_interfaces::MIGRATIONS[..1],
        ..lis_interfaces::SCHEMA
    };
    let modules = [
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        interfaces_v1,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
        lis_results::SCHEMA,
        lis_reporting::SCHEMA,
        lis_billing::SCHEMA,
        lis_ops::SCHEMA,
        lis_ops::RUNTIME_SCHEMA,
        lis_ops::ANCHORS_SCHEMA,
    ];
    let clock = Arc::new(FixedClock::new(start_time()));
    drop(Database::open(dir.path(), &modules, clock.clone()).unwrap());
    let at = start_time();
    rusqlite::Connection::open(dir.path().join(DATABASE_FILE))
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO ifc_instruments (id, code, name, connection_kind, tcp_bind_address,
                 tcp_port, tcp_allowed_peers, enabled, revision)
             VALUES ('{TCP_ID}', 'ASTM-TCP', 'Synthetic TCP', 'astm_tcp', '127.0.0.1', 4100,
                 '[\"127.0.0.1\"]', 0, 2);
             INSERT INTO ifc_receipts (id, instrument_id, transport, peer, started_at,
                 completed_at, state, frame_count, message_sha256, artifact_digest)
             VALUES ('{RECEIPT_ID}', '{TCP_ID}', 'astm_tcp', '127.0.0.1:50000', '{at}', '{at}',
                 'complete', 1, '{DIGEST}', '{DIGEST}');
             INSERT INTO ana_instrument_status_events (id, instrument_id, status, reason,
                 recorded_by, recorded_at, seq)
             VALUES ('01890a5d-ac96-774b-bcce-b302099a8070', '{TCP_ID}', 'in_service',
                 'Synthetic', 'synthetic-user', '{at}', 1);"
        ))
        .unwrap();

    let engine = Engine::start(EngineConfig {
        data_dir: dir.path().to_owned(),
        clock,
        keys: KeyProvider::Fixed(DataKey::from_bytes([7; 32])),
        printer_destinations: PrinterDestinations::LabNetworkAndLoopback,
        webview: None,
        outbox_handlers: Vec::new(),
        installation_kind: InstallationKind::Laboratory,
    })
    .await
    .unwrap();
    let status = engine.list_listener_status().await.unwrap();
    assert_eq!(status[0].instrument_name, "Synthetic TCP");
    engine.shutdown().await;
    drop(engine);

    let conn = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let kept: (String, Option<String>, String, i64) = conn
        .query_row(
            "SELECT i.connection_kind, i.result_folder, r.state, i.revision
             FROM ifc_instruments i JOIN ifc_receipts r ON r.instrument_id = i.id
             JOIN ana_instrument_status_events e ON e.instrument_id = i.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        kept,
        ("astm_tcp".to_owned(), None, "complete".to_owned(), 2)
    );
    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}
