use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use secrecy::ExposeSecret;
use secrecy::SecretString;
use serde::Serialize;
use tokio::sync::Notify;

use crate::SecurityKeyError;

/// Progress events contain neither names nor authenticator secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyStatus {
    /// Insert/select the key and touch it when its light flashes.
    WaitingForTouch,
    /// Submit one PIN using the associated operation's `KeyControl`.
    PinRequired,
    /// The key is processing the operation.
    Processing,
}

pub(crate) struct InteractionState {
    cancelled: AtomicBool,
    pin_requested: AtomicBool,
    notify: Notify,
    deadline: Instant,
}

/// A handle for cancelling a single ceremony or answering its current PIN prompt.
#[derive(Clone)]
pub struct KeyControl {
    shared: Arc<InteractionState>,
    pins: mpsc::SyncSender<SecretString>,
}

/// Native-side state for one ceremony. It must never be serialized or logged.
pub struct KeyInteraction {
    pub(crate) shared: Arc<InteractionState>,
    pins: Mutex<mpsc::Receiver<SecretString>>,
    status: mpsc::Sender<KeyStatus>,
}

impl fmt::Debug for KeyInteraction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KeyInteraction(redacted)")
    }
}

impl Drop for KeyInteraction {
    fn drop(&mut self) {
        self.shared.pin_requested.store(false, Ordering::SeqCst);
        // Drop any queued SecretString now, including cancellation during delivery.
        if let Ok(receiver) = self.pins.lock() {
            while receiver.try_recv().is_ok() {}
        }
    }
}

/// Begin a sixty-second ceremony. Forward progress to the requesting window only.
pub fn interaction() -> (KeyInteraction, KeyControl, mpsc::Receiver<KeyStatus>) {
    let (pins, pin_receiver) = mpsc::sync_channel(1);
    let (status, status_receiver) = mpsc::channel();
    let shared = Arc::new(InteractionState {
        cancelled: AtomicBool::new(false),
        pin_requested: AtomicBool::new(false),
        notify: Notify::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    });
    let control = KeyControl {
        shared: Arc::clone(&shared),
        pins,
    };
    let interaction = KeyInteraction {
        shared,
        pins: Mutex::new(pin_receiver),
        status,
    };
    (interaction, control, status_receiver)
}

impl KeyControl {
    /// Invalidate the ceremony; any late native response is discarded.
    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::SeqCst);
        self.shared.pin_requested.store(false, Ordering::SeqCst);
        self.shared.notify.notify_waiters();
    }

    /// Deliver one PIN to a live prompt. The caller must clear its input field.
    pub fn submit_pin(&self, pin: SecretString) -> Result<(), SecurityKeyError> {
        self.shared.check()?;
        if !self.shared.pin_requested.swap(false, Ordering::SeqCst) {
            return Err(SecurityKeyError::PinNotRequested);
        }
        self.pins
            .try_send(pin)
            .map_err(|_| SecurityKeyError::PinNotRequested)
    }
}

impl InteractionState {
    pub(crate) fn check(&self) -> Result<(), SecurityKeyError> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(SecurityKeyError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(SecurityKeyError::TimedOut);
        }
        Ok(())
    }

    pub(crate) async fn stopped(&self) -> SecurityKeyError {
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if let Err(error) = self.check() {
            return error;
        }
        tokio::select! {
            _ = notified => SecurityKeyError::Cancelled,
            _ = tokio::time::sleep_until(self.deadline.into()) => SecurityKeyError::TimedOut,
        }
    }
}

impl KeyInteraction {
    pub(crate) fn status(&self, status: KeyStatus) {
        let _ = self.status.send(status);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn request_pin(&self) -> Option<String> {
        self.shared.check().ok()?;
        let receiver = self.pins.lock().ok()?;
        self.shared.pin_requested.store(true, Ordering::SeqCst);
        self.status(KeyStatus::PinRequired);
        loop {
            if self.shared.check().is_err() {
                self.shared.pin_requested.store(false, Ordering::SeqCst);
                return None;
            }
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(pin) => return Some(pin.expose_secret().to_owned()),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsolicited_and_cancelled_pin_submission_is_refused() {
        let (_interaction, control, _events) = interaction();
        assert_eq!(
            control.submit_pin("synthetic".into()),
            Err(SecurityKeyError::PinNotRequested)
        );
        control.cancel();
        assert_eq!(
            control.submit_pin("synthetic".into()),
            Err(SecurityKeyError::Cancelled)
        );
    }

    #[tokio::test]
    async fn cancellation_before_wait_is_observed() {
        let (interaction, control, _events) = interaction();
        control.cancel();
        assert_eq!(
            interaction.shared.stopped().await,
            SecurityKeyError::Cancelled
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn pin_is_accepted_once_only_for_the_waiting_operation() {
        let (interaction, control, events) = interaction();
        let worker = std::thread::spawn(move || interaction.request_pin());
        assert_eq!(events.recv().unwrap(), KeyStatus::PinRequired);
        control.submit_pin("synthetic-pin".into()).unwrap();
        assert_eq!(worker.join().unwrap().as_deref(), Some("synthetic-pin"));
        assert_eq!(
            control.submit_pin("stale-pin".into()),
            Err(SecurityKeyError::PinNotRequested)
        );
    }
}
