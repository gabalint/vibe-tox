//! The poller's task: a scan at engine start, then one every interval, at the sign-in
//! screen too (F13).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::ResultFolders;
use crate::engine::Engine;

/// The poller's task and its stop signal. Dropping it, as a dropped engine does, also
/// stops the task after its current scan.
pub(crate) struct FolderPoller {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl FolderPoller {
    /// Scans now, then every `interval`. Must run inside a tokio runtime.
    pub(crate) fn start(folders: Arc<ResultFolders>, interval: Duration) -> Self {
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(poll(folders, stopped, interval));
        Self { stop, task }
    }

    /// Stops after the current scan, which stops between files.
    pub(crate) async fn stop(self, folders: &ResultFolders) {
        folders.cancel();
        // The receiver is gone if the task already ended.
        let _ = self.stop.send(true);
        if self.task.await.is_err() {
            tracing::error!("result folder poller ended with a panic");
        }
    }
}

/// The poller's loop: a scan, then a wait for `interval` or the stop signal.
async fn poll(folders: Arc<ResultFolders>, mut stopped: watch::Receiver<bool>, interval: Duration) {
    while !*stopped.borrow() {
        scan_off_runtime(Arc::clone(&folders)).await;
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

/// One scan on a blocking thread, since it waits on the file system and the database.
async fn scan_off_runtime(folders: Arc<ResultFolders>) {
    if tokio::task::spawn_blocking(move || folders.scan())
        .await
        .is_err()
    {
        tracing::error!("result folder scan stopped unexpectedly");
    }
}

impl Engine {
    /// One watched-folder scan now, as the poller runs every 10 seconds from start, at the
    /// sign-in screen too (F13). Scenario tests drive the poller with it and the clock.
    pub async fn scan_result_folders(&self) {
        scan_off_runtime(Arc::clone(&self.result_folders)).await;
    }
}
