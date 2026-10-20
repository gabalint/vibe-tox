//! The engine's two operations loops (REQ-OPS-007, REQ-OPS-008, REQ-OPS-013, NFR-26; M4
//! design brief D7, §3.4, §4, §6):
//!
//! - **Light loop**, every 15 s: the app run's heartbeat, wall-clock jump detection
//!   (`ops.clock_change`), the keep-awake controller, and health evaluation every 5
//!   minutes with history rows on change ([`light`], [`health`]).
//! - **Job runner**, one job at a time: requested backups first, then due backups, seals
//!   and anchor export every 15 minutes and after each backup, requested restore
//!   tests and exit exports, and the daily chain and anchor verification ([`jobs`]). Its
//!   first pass catches up what is overdue. A cancellation flag stops a running job at
//!   shutdown, which then ends `interrupted`.
//!
//! Also here: the operations steps of startup ([`startup`]) and the close warning with
//! the app run's stop record ([`quit`]).
//!
//! Both loops share [`Operations`] with the engine and act as system principals. Seals,
//! heartbeats and health transitions are technical writes; nothing here calls a network
//! service (REQ-OPS-009).

mod health;
mod jobs;
mod light;
mod quit;
pub(crate) mod startup;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use lis_core::Clock;
use lis_interfaces::ListenerSupervisor;
use lis_ops::exit_export::ExitExportId;
use lis_ops::runs::AppRunId;
use lis_store::Database;
use tokio::sync::{Notify, watch};
use tokio::task::JoinHandle;

use crate::keep_awake::{Inhibitor, KeepAwake, KeepAwakeView};
use crate::keys::InstallationKey;
use crate::pipeline::{Failure, ModuleError};

pub(crate) use health::{HealthCache, current as current_health, evaluate_and_record};
pub use health::{HostHealthView, HostView};
pub(crate) use jobs::{JobKind, run_pass as run_jobs};
pub use quit::QuitWarnings;

/// How often the light loop runs.
const LIGHT_INTERVAL: Duration = Duration::from_secs(15);
/// How often the job runner looks for work when nothing wakes it.
const JOB_POLL: Duration = Duration::from_secs(60);

/// Operations state shared by the engine and its two loops.
pub(crate) struct Operations {
    pub(crate) db: Arc<Database>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) listeners: Arc<ListenerSupervisor>,
    pub(crate) data_dir: PathBuf,
    /// This start's app run.
    pub(crate) run_id: AppRunId,
    /// The web engine reported at start.
    pub(crate) webview: Option<String>,
    /// The installation key and its state (§3.9).
    pub(crate) key: Mutex<InstallationKey>,
    /// Set once `ops.app_stop` is recorded, so it is recorded once.
    pub(crate) stopped: AtomicBool,
    /// Raised at shutdown; a running job stops between files and chunks.
    pub(crate) cancel: AtomicBool,
    /// The job running now, for the close warning.
    pub(crate) running: Mutex<Option<JobKind>>,
    /// Wakes the job runner after a request.
    pub(crate) wake: Notify,
    /// Exit exports requested with the folder their dialog returned, oldest first.
    pub(crate) exit_exports: Mutex<VecDeque<(ExitExportId, PathBuf)>>,
    pub(crate) keep_awake: Mutex<Option<KeepAwake>>,
    pub(crate) health: Mutex<HealthCache>,
    pub(crate) light: Mutex<light::LightState>,
    pub(crate) schedule: Mutex<jobs::Schedule>,
}

/// Why an operations step failed. Messages are PHI-free and name no path.
#[derive(Debug, thiserror::Error)]
pub(crate) enum OpsError {
    #[error(transparent)]
    Runtime(#[from] lis_ops::RuntimeError),
    #[error(transparent)]
    Backup(#[from] lis_ops::backup::BackupError),
    #[error(transparent)]
    Anchor(#[from] lis_ops::anchors::AnchorError),
    #[error(transparent)]
    Audit(#[from] lis_audit::AuditError),
    #[error(transparent)]
    Store(#[from] lis_store::StoreError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("a blocking operations step stopped unexpectedly")]
    Task,
}

impl ModuleError for OpsError {
    fn denied(&self) -> Option<lis_core::Denied> {
        match self {
            Self::Store(error) => error.denied(),
            Self::Runtime(lis_ops::RuntimeError::Store(error)) => error.denied(),
            _ => None,
        }
    }
}

impl From<OpsError> for Failure {
    fn from(error: OpsError) -> Self {
        match error {
            OpsError::Runtime(error) => error.into(),
            OpsError::Store(error) => error.into(),
            other => Self::Internal(other.to_string()),
        }
    }
}

/// The loops' tasks and the stop signal.
pub(crate) struct Loops {
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

/// Locks `mutex`; a poisoned lock still holds a coherent value.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Operations {
    #[expect(
        clippy::too_many_arguments,
        reason = "the engine parts the loops share, each kept as its own field"
    )]
    pub(crate) fn new(
        db: Arc<Database>,
        clock: Arc<dyn Clock>,
        listeners: Arc<ListenerSupervisor>,
        data_dir: PathBuf,
        run_id: AppRunId,
        webview: Option<String>,
        key: InstallationKey,
    ) -> Self {
        Self {
            db,
            clock,
            listeners,
            data_dir,
            run_id,
            webview,
            key: Mutex::new(key),
            stopped: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            running: Mutex::new(None),
            wake: Notify::new(),
            exit_exports: Mutex::new(VecDeque::new()),
            keep_awake: Mutex::new(None),
            health: Mutex::new(HealthCache::default()),
            light: Mutex::new(light::LightState::default()),
            schedule: Mutex::new(jobs::Schedule::default()),
        }
    }

    /// Starts the keep-awake controller and both loops.
    pub(crate) fn start(self: &Arc<Self>, inhibitor: Inhibitor) -> Loops {
        *locked(&self.keep_awake) = Some(KeepAwake::start(inhibitor, Arc::clone(&self.clock)));
        let (stop, stopped) = watch::channel(false);
        let light = tokio::spawn(every(
            Arc::clone(self),
            stopped.clone(),
            (LIGHT_INTERVAL, false),
            |ops| async move { light::tick(&ops).await },
        ));
        let jobs = tokio::spawn(every(
            Arc::clone(self),
            stopped,
            (JOB_POLL, true),
            |ops| async move { jobs::run_pass(&ops).await },
        ));
        Loops {
            stop,
            tasks: vec![light, jobs],
        }
    }

    /// Raises the cancellation flag, stops both loops after their current step, then
    /// releases the inhibitor.
    pub(crate) async fn stop(&self, loops: Loops) {
        self.cancel.store(true, Ordering::SeqCst);
        // The receivers may already be gone if a loop stopped.
        let _ = loops.stop.send(true);
        for task in loops.tasks {
            if task.await.is_err() {
                tracing::error!("an operations loop ended with a panic");
            }
        }
        let keep_awake = locked(&self.keep_awake).take();
        if let Some(keep_awake) = keep_awake {
            // Joining waits for the release on the controller's own thread.
            if tokio::task::spawn_blocking(move || keep_awake.stop())
                .await
                .is_err()
            {
                tracing::error!("keep-awake controller did not stop cleanly");
            }
        }
    }

    /// The keep-awake state; `not_needed` before the loops start.
    pub(crate) fn keep_awake(&self) -> KeepAwakeView {
        locked(&self.keep_awake).as_ref().map_or(
            KeepAwakeView {
                state: lis_ops::health::KeepAwakeState::NotNeeded,
                since: None,
                code: None,
            },
            KeepAwake::status,
        )
    }

    /// Queues an exit export into `folder` and wakes the job runner.
    pub(crate) fn queue_exit_export(&self, export_id: ExitExportId, folder: PathBuf) {
        locked(&self.exit_exports).push_back((export_id, folder));
        self.wake.notify_one();
    }
}

/// Runs `step` now and then every `interval` (or, when `woken`, also after a request),
/// until `stop` turns true.
async fn every<F, Fut>(
    ops: Arc<Operations>,
    mut stop: watch::Receiver<bool>,
    (interval, woken): (Duration, bool),
    step: F,
) where
    F: Fn(Arc<Operations>) -> Fut,
    Fut: Future<Output = ()>,
{
    while !*stop.borrow() {
        step(Arc::clone(&ops)).await;
        tokio::select! {
            changed = stop.changed() => {
                // A dropped sender also means stop.
                if changed.is_err() {
                    return;
                }
            }
            () = ops.wake.notified(), if woken => {}
            () = tokio::time::sleep(interval) => {}
        }
    }
}
