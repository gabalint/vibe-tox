//! Sleep prevention while analyzer listeners are active (REQ-OPS-013; M4 design brief
//! §6.2). A dedicated thread acquires and releases the OS inhibitor, because Windows
//! `SetThreadExecutionState` is per thread. The light loop says whether it is wanted: while
//! any listener is listening, connected or receiving. The inhibitor keeps the system awake
//! (`idle`), not the display, and never forces sleep off on its own (`sleep`).
//!
//! States: `held`, `not_needed`, or `unavailable` with a PHI-free code (for example in a
//! container without logind). Tests inject the inhibitor.

use std::any::Any;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use lis_core::{Clock, UtcTimestamp};
use lis_ops::health::KeepAwakeState;
use serde::Serialize;
use ts_rs::TS;

/// Why sleep is prevented, as the OS shows it.
const REASON: &str = "Receiving analyzer results";
/// The code shown when the OS refused the inhibitor.
const REFUSED: &str = "inhibitor_refused";

/// What the inhibitor returns: a guard whose drop releases it.
pub type Guard = Box<dyn Any>;
type Acquire = Box<dyn FnMut() -> Result<Guard, String> + Send>;

/// Acquires the OS sleep inhibitor on the controller's thread.
pub struct Inhibitor(Acquire);

impl Inhibitor {
    /// The OS inhibitor (`keepawake`).
    #[must_use]
    pub fn system() -> Self {
        Self::new(|| {
            keepawake::Builder::default()
                .idle(true)
                .display(false)
                .sleep(false)
                .reason(REASON)
                .app_name("Vibe Toxicology LIS")
                .app_reverse_domain("org.vibetox.lis")
                .create()
                .map(|guard| Box::new(guard) as Guard)
                .map_err(|error| error.to_string())
        })
    }

    /// An injected inhibitor, for tests: `acquire` returns a guard or an error text.
    pub fn new(acquire: impl FnMut() -> Result<Guard, String> + Send + 'static) -> Self {
        Self(Box::new(acquire))
    }
}

/// The inhibitor's state as health shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeepAwakeView {
    /// Held, not needed or unavailable.
    pub state: KeepAwakeState,
    /// Since when; `None` before the first change.
    pub since: Option<UtcTimestamp>,
    /// Why it is unavailable.
    pub code: Option<&'static str>,
}

/// The controller: a thread that holds the inhibitor while it is wanted.
pub(crate) struct KeepAwake {
    wanted: Sender<bool>,
    status: Arc<Mutex<KeepAwakeView>>,
    thread: Option<JoinHandle<()>>,
}

impl KeepAwake {
    /// Starts the controller thread; nothing is held until [`want`](Self::want) says so.
    pub(crate) fn start(inhibitor: Inhibitor, clock: Arc<dyn Clock>) -> Self {
        let (wanted, requests) = mpsc::channel();
        let status = Arc::new(Mutex::new(KeepAwakeView {
            state: KeepAwakeState::NotNeeded,
            since: None,
            code: None,
        }));
        let shared = Arc::clone(&status);
        let thread = std::thread::Builder::new()
            .name("keep-awake".to_owned())
            .spawn(move || control(inhibitor, &requests, &shared, clock.as_ref()))
            .inspect_err(|error| tracing::error!(%error, "keep-awake thread did not start"))
            .ok();
        if thread.is_none() {
            set(&status, KeepAwakeState::Unavailable, None, Some(REFUSED));
        }
        Self {
            wanted,
            status,
            thread,
        }
    }

    /// Holds the inhibitor while `active`, releases it otherwise.
    pub(crate) fn want(&self, active: bool) {
        // A stopped controller has nothing left to hold.
        let _ = self.wanted.send(active);
    }

    /// The current state.
    pub(crate) fn status(&self) -> KeepAwakeView {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Releases the inhibitor and stops the thread.
    pub(crate) fn stop(mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        drop(self.wanted);
        if thread.join().is_err() {
            tracing::error!("keep-awake thread ended with a panic");
        }
    }
}

/// The thread: acquires on `true`, releases on `false`, and releases when the controller
/// is dropped.
fn control(
    mut inhibitor: Inhibitor,
    requests: &Receiver<bool>,
    status: &Mutex<KeepAwakeView>,
    clock: &dyn Clock,
) {
    let mut guard: Option<Guard> = None;
    while let Ok(active) = requests.recv() {
        match (active, guard.is_some()) {
            (true, false) => match (inhibitor.0)() {
                Ok(held) => {
                    guard = Some(held);
                    set(status, KeepAwakeState::Held, Some(clock.now()), None);
                    tracing::info!("sleep prevented while analyzer listeners are active");
                }
                Err(error) => {
                    let current = status.lock().unwrap_or_else(PoisonError::into_inner).state;
                    if current != KeepAwakeState::Unavailable {
                        tracing::warn!(%error, "sleep prevention unavailable");
                        set(
                            status,
                            KeepAwakeState::Unavailable,
                            Some(clock.now()),
                            Some(REFUSED),
                        );
                    }
                }
            },
            (false, true) => {
                guard = None;
                set(status, KeepAwakeState::NotNeeded, Some(clock.now()), None);
                tracing::info!("sleep prevention released");
            }
            (false, false) => {
                let current = status.lock().unwrap_or_else(PoisonError::into_inner).state;
                if current == KeepAwakeState::Unavailable {
                    set(status, KeepAwakeState::NotNeeded, Some(clock.now()), None);
                }
            }
            (true, true) => {}
        }
    }
    drop(guard);
}

fn set(
    status: &Mutex<KeepAwakeView>,
    state: KeepAwakeState,
    since: Option<UtcTimestamp>,
    code: Option<&'static str>,
) {
    *status.lock().unwrap_or_else(PoisonError::into_inner) = KeepAwakeView { state, since, code };
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use lis_core::FixedClock;

    use super::*;

    /// A guard that counts its release.
    struct Counted(Arc<AtomicUsize>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn clock() -> Arc<dyn Clock> {
        Arc::new(FixedClock::new(
            UtcTimestamp::parse("2026-10-13T07:00:00.000000000Z").unwrap(),
        ))
    }

    fn wait_for(keep_awake: &KeepAwake, state: KeepAwakeState) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while keep_awake.status().state != state {
            assert!(
                Instant::now() < deadline,
                "state stayed {:?}",
                keep_awake.status()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn holds_while_wanted_and_releases_once() {
        let (acquired, released) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (counter, release) = (Arc::clone(&acquired), Arc::clone(&released));
        let keep_awake = KeepAwake::start(
            Inhibitor::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Counted(Arc::clone(&release))) as Guard)
            }),
            clock(),
        );
        assert_eq!(keep_awake.status().state, KeepAwakeState::NotNeeded);
        keep_awake.want(true);
        keep_awake.want(true);
        wait_for(&keep_awake, KeepAwakeState::Held);
        keep_awake.want(false);
        wait_for(&keep_awake, KeepAwakeState::NotNeeded);
        assert_eq!(released.load(Ordering::SeqCst), 1);
        keep_awake.want(true);
        wait_for(&keep_awake, KeepAwakeState::Held);
        keep_awake.stop();
        assert_eq!(acquired.load(Ordering::SeqCst), 2);
        assert_eq!(
            released.load(Ordering::SeqCst),
            2,
            "stop releases the inhibitor"
        );
    }

    #[test]
    fn a_refused_inhibitor_is_unavailable_with_a_code() {
        let keep_awake = KeepAwake::start(Inhibitor::new(|| Err("no logind".to_owned())), clock());
        keep_awake.want(true);
        wait_for(&keep_awake, KeepAwakeState::Unavailable);
        assert_eq!(keep_awake.status().code, Some(REFUSED));
        keep_awake.want(false);
        wait_for(&keep_awake, KeepAwakeState::NotNeeded);
        keep_awake.stop();
    }
}
