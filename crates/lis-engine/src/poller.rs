//! A background task that runs a blocking pass at engine start and then every interval, at
//! the sign-in screen too: the watched result folders (F13) and the partner deliveries.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Work a [`Poller`] runs on a blocking thread.
pub(crate) trait Scan: Send + Sync + 'static {
    /// One pass. Blocking: file system and database work.
    fn scan(&self);
    /// Stops a pass in progress at its next safe point, for shutdown.
    fn cancel(&self);
}

/// The task and its stop signal. Dropping it, as a dropped engine does, also stops the task
/// after its current pass.
pub(crate) struct Poller {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
    scanner: Arc<dyn Scan>,
}

impl Poller {
    /// Runs a pass now, then every `interval`. Must run inside a tokio runtime.
    pub(crate) fn start(scanner: Arc<dyn Scan>, interval: Duration) -> Self {
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(poll(Arc::clone(&scanner), stopped, interval));
        Self {
            stop,
            task,
            scanner,
        }
    }

    /// Stops after the current pass, which stops at its next safe point.
    pub(crate) async fn stop(self) {
        self.scanner.cancel();
        // The receiver is gone if the task already ended.
        let _ = self.stop.send(true);
        if self.task.await.is_err() {
            tracing::error!("poller ended with a panic");
        }
    }
}

/// The loop: a pass, then a wait for `interval` or the stop signal.
async fn poll(scanner: Arc<dyn Scan>, mut stopped: watch::Receiver<bool>, interval: Duration) {
    while !*stopped.borrow() {
        scan_off_runtime(Arc::clone(&scanner)).await;
        tokio::select! {
            changed = stopped.changed() => {
                // A dropped sender also means stop.
                if changed.is_err() {
                    return;
                }
            }
            () = tokio::time::sleep(interval) => {}
        }
    }
}

/// One pass on a blocking thread, since it waits on the file system and the database.
pub(crate) async fn scan_off_runtime(scanner: Arc<dyn Scan>) {
    if tokio::task::spawn_blocking(move || scanner.scan())
        .await
        .is_err()
    {
        tracing::error!("poller pass stopped unexpectedly");
    }
}
