//! The engine: composition root, startup and shutdown (M4 design brief §6.4).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use lis_auth::{AuthError, ChallengeSlot, DataKey, EndReason, SecurityPolicy};
use lis_core::{Clock, CorrelationId, ModuleSchema, SessionId, UtcTimestamp, WorkstationId};
use lis_interfaces::ListenerSupervisor;
use lis_labels::PrinterDestinations;
use lis_ops::InstallationKind;
use lis_ops::exit_export::ExitExportId;
use lis_ops::runs::StopKind;
use lis_store::Database;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::commands::file_templates::LayoutSamples;
use crate::keep_awake::Inhibitor;
use crate::keys::KeyProvider;
use crate::ops_worker::{Loops, Operations, locked, startup};
use crate::outbox_worker::{Dispatcher, TopicHandler};
use crate::pipeline::Failure;
use crate::result_folders::{FolderPoller, ResultFolders};
use crate::session::SessionState;

mod error;
pub use error::EngineError;

/// Module schemas in crate-map order. The store applies its own and the audit
/// schema first.
pub(crate) const MODULES: &[ModuleSchema] = &[
    lis_auth::SCHEMA,
    lis_identity::SCHEMA,
    lis_catalog::SCHEMA,
    lis_interfaces::SCHEMA,
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

/// The app version recorded with each run (one workspace version for every crate).
pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the engine needs to start.
pub struct EngineConfig {
    /// The app data directory holding the database and artifacts.
    pub data_dir: PathBuf,
    /// The time source.
    pub clock: Arc<dyn Clock>,
    /// Where the installation key comes from.
    pub keys: KeyProvider,
    /// Which addresses label printers may have: the app passes
    /// [`PrinterDestinations::LabNetwork`]; only tests add loopback.
    pub printer_destinations: PrinterDestinations,
    /// Web engine name and version, recorded with each session and app run (ADR 0012).
    pub webview: Option<String>,
    /// Outbox topic handlers besides the built-in ones.
    pub outbox_handlers: Vec<TopicHandler>,
    /// Which installation the data folder holds; a database created as the other kind is
    /// refused at startup (proposed REQ-OPS-016).
    pub installation_kind: InstallationKind,
}

/// The running engine. Commands and queries are its methods.
pub struct Engine {
    pub(crate) db: Arc<Database>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) workstation_id: WorkstationId,
    pub(crate) webview: Option<String>,
    pub(crate) session: Mutex<SessionState>,
    /// The session whose start is known, and when it started.
    pub(crate) session_started: Mutex<Option<(SessionId, UtcTimestamp)>>,
    /// The security policy in force, cached (NFR-19).
    pub(crate) policy: Mutex<SecurityPolicy>,
    pub(crate) challenges: ChallengeSlot,
    pub(crate) key_ceremonies: crate::commands::security_keys::KeyCeremonies,
    /// Sample files open on the file template page, for the signed-in session only.
    pub(crate) layout_samples: LayoutSamples,
    pub(crate) keys: KeyProvider,
    /// Laboratory or demo, as opened (proposed REQ-OPS-016).
    pub(crate) installation_kind: InstallationKind,
    pub(crate) printer_destinations: PrinterDestinations,
    /// Operations state shared with the light loop and the job runner.
    pub(crate) ops: Arc<Operations>,
    dispatcher: Mutex<Option<(watch::Sender<bool>, JoinHandle<()>)>>,
    loops: Mutex<Option<Loops>>,
    pub(crate) listeners: Arc<ListenerSupervisor>,
    /// The watched-folder poller of result-file instruments, beside the listeners (F13).
    pub(crate) result_folders: Arc<ResultFolders>,
    folder_poller: Mutex<Option<FolderPoller>>,
    /// The runtime the engine started on. The shell calls [`Engine::start_operations`] from
    /// its setup hook, outside any runtime context, so the loops are spawned through it.
    runtime: tokio::runtime::Handle,
}

impl Engine {
    /// Starts the engine (§6.4), in this order:
    ///
    /// 1. refuse a data folder on a network file system or on a volume not known to be
    ///    local;
    /// 2. apply a confirmed restore, before the database opens;
    /// 3. open the database and apply migrations (the store keeps a pre-migration copy);
    /// 4. the installation row, refusing a database created as the other kind; clean
    ///    restore scratch folders;
    /// 5. record an applied restore, which opens the recovery reconciliation and raises
    ///    the number floors, or stop;
    /// 6. register this workstation and end sessions a crash left open;
    /// 7. load the installation key and decide its state, and complete the audit of a key
    ///    restore a crash or an audit failure left without its completion;
    /// 8. with a usable key, move published backup sets no successful copy records to
    ///    quarantine, before anything lists or prunes sets;
    /// 9. record the app run (`ops.app_start`: unclean previous stop, gap, version and
    ///    web-engine change) and mark interrupted backups, restore tests and exports;
    /// 10. reconcile analyzer receipts, start the listeners and the watched-folder poller,
    ///     whose first scan runs now;
    /// 11. start the outbox dispatcher, which holds label printing while a recovery
    ///     reconciliation is open.
    ///
    /// The keep-awake controller, the light loop and the job runner start with
    /// [`start_operations`](Self::start_operations). Must run inside a tokio runtime.
    pub async fn start(config: EngineConfig) -> Result<Self, EngineError> {
        let data_dir = config.data_dir;
        local_data_folder(lis_ops::host::data_folder_on_network(&data_dir))?;
        startup::apply_restore(&data_dir, config.clock.now()).await?;
        let db = Arc::new(Database::open(
            &data_dir,
            MODULES,
            Arc::clone(&config.clock),
        )?);
        let kind = config.installation_kind;
        startup::lifecycle_write(&db, move |ctx| lis_ops::open_installation(ctx, kind)).await?;
        if let Err(error) = lis_ops::restore::clean_scratch(&data_dir) {
            tracing::warn!(%error, "restore scratch folders not cleaned");
        }
        startup::record_restore(&db, &data_dir).await?;
        let (workstation_id, abandoned) =
            startup::lifecycle_write(&db, |ctx| -> Result<_, AuthError> {
                let workstation_id = lis_auth::register_workstation(ctx)?;
                Ok((workstation_id, lis_auth::end_abandoned_sessions(ctx)?))
            })
            .await?;
        let key = {
            let (db, keys) = (Arc::clone(&db), config.keys.clone());
            tokio::task::spawn_blocking(move || crate::keys::resolve(&db, &keys))
                .await
                .map_err(|_| EngineError::StartupTask)?
                .map_err(|error| EngineError::InstallationKey(error.to_string()))?
        };
        crate::commands::operations::reconcile_key_restores(&db, key.state).await?;
        startup::reconcile_published_sets(&db, key.usable().ok()).await;
        let run = startup::start_run(&db, config.webview.clone()).await?;
        let policy = db.read(lis_auth::current_security_policy).await?;
        let listeners = Arc::new(ListenerSupervisor::new(Arc::clone(&db)));
        start_listeners(Arc::clone(&db), Arc::clone(&listeners)).await?;
        let (result_folders, folder_poller) = ResultFolders::start(&db, &config.clock, &listeners);
        let dispatcher = start_dispatcher(
            &db,
            config.outbox_handlers,
            config.printer_destinations,
            crate::demo::label_marking(config.installation_kind),
        );
        tracing::info!(
            %workstation_id,
            abandoned_sessions = abandoned,
            key_state = key.state.as_str(),
            "engine started"
        );
        let ops = Arc::new(Operations::new(
            Arc::clone(&db),
            Arc::clone(&config.clock),
            Arc::clone(&listeners),
            data_dir,
            run,
            config.webview.clone(),
            key,
        ));
        Ok(Self {
            db,
            clock: config.clock,
            workstation_id,
            webview: config.webview,
            session: Mutex::new(SessionState::None),
            session_started: Mutex::new(None),
            policy: Mutex::new(policy),
            challenges: ChallengeSlot::default(),
            key_ceremonies: crate::commands::security_keys::KeyCeremonies::default(),
            layout_samples: LayoutSamples::default(),
            keys: config.keys,
            installation_kind: config.installation_kind,
            printer_destinations: config.printer_destinations,
            ops,
            dispatcher: Mutex::new(Some(dispatcher)),
            loops: Mutex::new(None),
            listeners,
            result_folders,
            folder_poller: Mutex::new(Some(folder_poller)),
            runtime: tokio::runtime::Handle::current(),
        })
    }

    /// Starts the keep-awake controller with `inhibitor`, the light loop and the job
    /// runner (§6.4 step 11); the runner's first pass catches up overdue backups, seals
    /// and anchors. The shell calls it right after [`start`](Self::start); scenario tests
    /// drive the same steps with [`run_operations_pass`](Self::run_operations_pass).
    /// Calling it again does nothing.
    pub fn start_operations(&self, inhibitor: Inhibitor) {
        let mut loops = self.loops.lock().unwrap_or_else(PoisonError::into_inner);
        if loops.is_none() {
            let _runtime = self.runtime.enter();
            *loops = Some(self.ops.start(inhibitor));
        }
    }

    /// One job-runner pass now: requested and due backups, seals and anchors, requested
    /// restore tests and exit exports, and the daily verification.
    pub async fn run_operations_pass(&self) {
        crate::ops_worker::run_jobs(&self.ops).await;
    }

    /// Wakes the job runner, after a request it should not wait a minute for.
    pub(crate) fn wake_job_runner(&self) {
        self.ops.wake.notify_one();
    }

    /// Queues an exit export into the `folder` its dialog returned, for the job runner.
    pub fn queue_exit_export(&self, export_id: ExitExportId, folder: PathBuf) {
        self.ops.queue_exit_export(export_id, folder);
    }

    /// Stops in the reverse order of [`start`](Self::start): the operations loops (a
    /// running job ends `interrupted`) and the inhibitor, the dispatcher after its
    /// current item, the watched-folder poller after its current scan, the analyzer
    /// listeners, then records `ops.app_stop` and ends the open
    /// session as `app_exit`. The database closes when the engine is dropped. Calling it
    /// again does nothing more.
    pub async fn shutdown(&self) {
        let loops = self
            .loops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(loops) = loops {
            self.ops.stop(loops).await;
        }
        self.ops.cancel.store(true, Ordering::SeqCst);
        self.stop_dispatcher().await;
        let folder_poller = self
            .folder_poller
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(folder_poller) = folder_poller {
            folder_poller.stop(&self.result_folders).await;
        }
        let listeners = Arc::clone(&self.listeners);
        match tokio::task::spawn_blocking(move || listeners.stop_all()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::error!(%error, "analyzer listeners did not stop cleanly"),
            Err(_) => tracing::error!("analyzer listener shutdown stopped unexpectedly"),
        }
        self.record_stop(StopKind::Clean).await;
        self.end_open_session().await;
        tracing::info!("engine stopped");
    }

    /// Stops the dispatcher after its current item.
    async fn stop_dispatcher(&self) {
        let dispatcher = self
            .dispatcher
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some((stop, dispatcher)) = dispatcher {
            // The receiver may already be gone if the dispatcher stopped.
            let _ = stop.send(true);
            if dispatcher.await.is_err() {
                tracing::error!("outbox dispatcher ended with a panic");
            }
        }
    }

    /// Ends the open session, if any, as `app_exit`.
    async fn end_open_session(&self) {
        let open = {
            let mut state = self.session_state();
            self.key_ceremonies.invalidate();
            match std::mem::take(&mut *state) {
                SessionState::None => None,
                SessionState::Active(session) | SessionState::Locked(session) => Some(session),
            }
        };
        let Some(session) = open else {
            return;
        };
        let ended: Result<(), AuthError> = self
            .db
            .write(
                self.user_principal(session, []),
                CorrelationId::generate(),
                move |ctx| lis_auth::end_session(ctx, session.id, EndReason::AppExit),
            )
            .await;
        if let Err(error) = ended {
            tracing::error!(%error, "session not ended at shutdown");
        }
    }

    /// The installation key for TOTP sign-in, or the typed refusal while it is missing,
    /// mismatched or unavailable (§3.9).
    pub(crate) async fn data_key(&self) -> Result<DataKey, Failure> {
        locked(&self.ops.key).usable()
    }
}

/// Every column holding artifact digests, as `(table, column)`, from their owners.
pub(crate) fn artifact_columns() -> Vec<(&'static str, &'static str)> {
    [
        lis_catalog::ARTIFACT_COLUMNS,
        lis_interfaces::ARTIFACT_COLUMNS,
        lis_reporting::ARTIFACT_COLUMNS,
        lis_billing::ARTIFACT_COLUMNS,
    ]
    .concat()
}

/// Starts the outbox dispatcher with the built-in handlers; labels go only to
/// printers `printer_destinations` admits, with the installation's `label_marking`.
fn start_dispatcher(
    db: &Arc<Database>,
    mut handlers: Vec<TopicHandler>,
    printer_destinations: PrinterDestinations,
    label_marking: lis_labels::Marking,
) -> (watch::Sender<bool>, JoinHandle<()>) {
    let (stop, stopped) = watch::channel(false);
    // Wave 3+: built-in outbox handlers, one block per area (for example label
    // printing, result import). Edit only your block.
    // specimens: begin
    handlers.push(crate::specimen_labels::handler(
        Arc::clone(db),
        printer_destinations,
        label_marking,
    ));
    // specimens: end
    // results: begin
    handlers.extend(crate::result_import::handlers(Arc::clone(db)));
    // results: end
    let dispatcher = Dispatcher::new(Arc::clone(db), handlers).spawn(stopped);
    (stop, dispatcher)
}

/// Recovers or marks interrupted the receipts a previous run left open, then
/// starts every enabled analyzer listener (REQ-OPS-013, REQ-INT-009). Both
/// block on the database, so they run off the async runtime. A listener that
/// cannot open is reported in its status, not as a startup failure.
async fn start_listeners(
    db: Arc<Database>,
    listeners: Arc<ListenerSupervisor>,
) -> Result<(), EngineError> {
    tokio::task::spawn_blocking(move || {
        let reconciled = lis_interfaces::reconcile_receipts(&db)?;
        tracing::info!(
            recovered = reconciled.recovered,
            interrupted = reconciled.interrupted,
            "receipt reconciliation finished"
        );
        if let Err(error) = listeners.start_all() {
            tracing::warn!(%error, "some analyzer listeners did not start; see listener status");
        }
        Ok(())
    })
    .await
    .map_err(|_| EngineError::StartupTask)?
}

/// D15: the app starts only when the probe says the data folder is on a local volume; a
/// network volume and an unknown one are refused with their own codes.
fn local_data_folder(on_network: Option<bool>) -> Result<(), EngineError> {
    match on_network {
        Some(false) => Ok(()),
        Some(true) => Err(EngineError::DataFolderOnNetwork),
        None => Err(EngineError::DataFolderUnknown),
    }
}

#[cfg(test)]
mod tests {
    use lis_store::{MigrationProblem, StoreError};

    use super::*;

    #[test]
    fn startup_failures_map_to_the_codes_the_screen_explains() {
        let migration = |problem| {
            EngineError::Store(StoreError::Migration {
                module: "ops".to_owned(),
                version: 9,
                problem,
            })
        };
        let cases = [
            (EngineError::DataFolderOnNetwork, "data_folder_on_network"),
            (EngineError::DataFolderUnknown, "data_folder_unknown"),
            (
                EngineError::Store(StoreError::WalUnavailable),
                "data_folder_on_network",
            ),
            (EngineError::Store(StoreError::AlreadyOpen), "already_open"),
            (
                migration(MigrationProblem::Missing),
                "database_newer_than_app",
            ),
            (
                migration(MigrationProblem::ChecksumChanged),
                "database_unreadable",
            ),
            (
                EngineError::Store(StoreError::PreMigrationSnapshotNotIntact),
                "pre_migration_snapshot_failed",
            ),
            (
                EngineError::RestoreRecord("synthetic".to_owned()),
                "restore_record_failed",
            ),
            (EngineError::StartupTask, "internal"),
            (
                EngineError::InstallationKindMismatch {
                    expected: "laboratory",
                    found: "demo",
                },
                "installation_kind_mismatch",
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code, "{error}");
        }
    }

    // D15: an unclassified volume is refused, not accepted as local.
    #[test]
    fn only_a_volume_known_to_be_local_starts() {
        assert!(local_data_folder(Some(false)).is_ok());
        assert!(matches!(
            local_data_folder(Some(true)),
            Err(EngineError::DataFolderOnNetwork)
        ));
        assert!(matches!(
            local_data_folder(None),
            Err(EngineError::DataFolderUnknown)
        ));
    }
}
