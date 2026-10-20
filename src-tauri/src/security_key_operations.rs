//! One native interaction at a time, bound to an opaque operation ID. No account
//! authority or WebAuthn proof is exposed through the progress/PIN control channel.

mod client;
mod types;

use types::Progress;
pub(crate) use types::{SecurityKeyOperationInput, SecurityKeyPinInput, SecurityKeyStatus};
#[cfg(test)]
mod tests;

use std::sync::{Mutex, PoisonError, mpsc};
use std::time::Duration;

use lis_core::{CommandError, CorrelationId, ErrorCode};
use lis_security_key::{KeyControl, KeyInteraction, KeyStatus, SecurityKeyError};
use tokio::sync::oneshot;

/// Shell-only progress state. The engine owns all authentication state.
#[derive(Default)]
pub(crate) struct SecurityKeyOperations(Mutex<Option<ActiveOperation>>);

struct ActiveOperation {
    id: String,
    interaction_id: String,
    cancelled: bool,
    sealed: bool,
    status: Progress,
    native: Option<(KeyControl, mpsc::Receiver<KeyStatus>)>,
    authenticated: bool,
    continue_registration: Option<oneshot::Sender<()>>,
}

/// Reservation lives through the entire engine command, including its final write.
pub(crate) struct Operation<'a> {
    service: &'a SecurityKeyOperations,
    id: String,
}

impl SecurityKeyOperations {
    pub(crate) fn begin(&self) -> Result<Operation<'_>, CommandError> {
        let mut current = self.0.lock().map_err(|_| unavailable())?;
        if current.is_some() {
            return Err(conflict(
                "Finish or cancel the current security key operation first.",
            ));
        }
        let id = CorrelationId::generate().to_string();
        *current = Some(ActiveOperation {
            id: id.clone(),
            interaction_id: id.clone(),
            cancelled: false,
            sealed: false,
            status: Progress::Processing,
            native: None,
            authenticated: false,
            continue_registration: None,
        });
        Ok(Operation { service: self, id })
    }

    pub(crate) fn status(&self) -> Result<Option<SecurityKeyStatus>, CommandError> {
        let mut current = self.0.lock().map_err(|_| unavailable())?;
        Ok(current.as_mut().map(|active| {
            active.drain_status();
            SecurityKeyStatus {
                operation_id: active.interaction_id.clone(),
                status: active.status,
            }
        }))
    }

    pub(crate) fn submit_pin(&self, input: SecurityKeyPinInput) -> Result<(), CommandError> {
        self.with_operation(
            |active| active.interaction_id == input.operation_id,
            |active| {
                active.check()?;
                let (control, _) = active
                    .native
                    .as_ref()
                    .ok_or_else(|| native_error(SecurityKeyError::PinNotRequested))?;
                control.submit_pin(input.pin).map_err(native_error)?;
                active.drain_status();
                active.status = Progress::Processing;
                Ok(())
            },
        )
    }

    pub(crate) fn continue_registration(
        &self,
        input: &SecurityKeyOperationInput,
    ) -> Result<(), CommandError> {
        self.with_operation(
            |active| active.interaction_id == input.operation_id,
            |active| {
                active.check()?;
                let sender = active.continue_registration.take().ok_or_else(|| {
                    conflict("This operation is not waiting for a replacement key.")
                })?;
                sender
                    .send(())
                    .map_err(|()| conflict("This security key operation is no longer active."))?;
                active.status = Progress::Processing;
                Ok(())
            },
        )
    }

    pub(crate) fn cancel(&self, input: &SecurityKeyOperationInput) -> Result<(), CommandError> {
        self.with_operation(
            |active| active.interaction_id == input.operation_id,
            |active| {
                if active.sealed {
                    return Err(conflict(
                        "Security key verification has finished; the action is completing.",
                    ));
                }
                active.cancel();
                Ok(())
            },
        )
    }

    pub(crate) fn cancel_all(&self) {
        if let Some(active) = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            active.cancel();
        }
    }

    fn with_operation<T>(
        &self,
        matches: impl Fn(&ActiveOperation) -> bool,
        work: impl FnOnce(&mut ActiveOperation) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut current = self.0.lock().map_err(|_| unavailable())?;
        let active = current
            .as_mut()
            .filter(|active| matches(active))
            .ok_or_else(|| conflict("This security key operation is no longer active."))?;
        work(active)
    }
}

impl ActiveOperation {
    fn check(&self) -> Result<(), CommandError> {
        if self.cancelled {
            return Err(native_error(SecurityKeyError::Cancelled));
        }
        if self.sealed {
            return Err(conflict("Security key verification has already finished."));
        }
        Ok(())
    }

    fn cancel(&mut self) {
        self.cancelled = true;
        self.status = Progress::Processing;
        self.continue_registration.take();
        if let Some((control, _)) = &self.native {
            control.cancel();
        }
    }

    fn drain_status(&mut self) {
        if let Some((_, receiver)) = &self.native {
            while let Ok(status) = receiver.try_recv() {
                self.status = status.into();
            }
        }
    }
}

impl Operation<'_> {
    fn start_native(&self) -> Result<KeyInteraction, CommandError> {
        self.service.with_operation(
            |active| active.id == self.id,
            |active| {
                active.check()?;
                if active.native.is_some() {
                    return Err(conflict(
                        "A native security key interaction is already active.",
                    ));
                }
                let (interaction, control, events) = lis_security_key::interaction();
                active.interaction_id = CorrelationId::generate().to_string();
                active.native = Some((control, events));
                active.status = Progress::WaitingForTouch;
                Ok(interaction)
            },
        )
    }

    fn end_native(&self) -> Result<(), CommandError> {
        self.service.with_operation(
            |active| active.id == self.id,
            |active| {
                active.native.take();
                active.status = Progress::Processing;
                active.check()
            },
        )
    }

    fn authenticated(&self) -> Result<(), CommandError> {
        self.service.with_operation(
            |active| active.id == self.id,
            |active| {
                active.check()?;
                active.authenticated = true;
                Ok(())
            },
        )
    }

    async fn await_replacement(&self) -> Result<(), CommandError> {
        let receiver = self.service.with_operation(
            |active| active.id == self.id,
            |active| {
                active.check()?;
                if !active.authenticated {
                    return Ok(None);
                }
                let (sender, receiver) = oneshot::channel();
                active.interaction_id = CorrelationId::generate().to_string();
                active.continue_registration = Some(sender);
                active.status = Progress::WaitingForReplacement;
                Ok(Some(receiver))
            },
        )?;
        if let Some(receiver) = receiver {
            tokio::time::timeout(Duration::from_secs(60), receiver)
                .await
                .map_err(|_| native_error(SecurityKeyError::TimedOut))?
                .map_err(|_| native_error(SecurityKeyError::Cancelled))?;
        }
        Ok(())
    }

    fn seal(&self) -> Result<(), CommandError> {
        self.service.with_operation(
            |active| active.id == self.id,
            |active| {
                active.check()?;
                if active.native.is_some() {
                    return Err(unavailable());
                }
                active.sealed = true;
                Ok(())
            },
        )
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        let mut current = self
            .service
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if current.as_ref().is_some_and(|active| active.id == self.id)
            && let Some(mut active) = current.take()
        {
            active.cancel();
        }
    }
}

fn conflict(message: &str) -> CommandError {
    boundary_error(ErrorCode::Conflict, message)
}

fn unavailable() -> CommandError {
    boundary_error(
        ErrorCode::Unavailable,
        "Security key interaction is unavailable. Restart the application.",
    )
}

fn native_error(error: SecurityKeyError) -> CommandError {
    boundary_error(ErrorCode::Validation, error.to_string())
}

fn boundary_error(code: ErrorCode, message: impl Into<String>) -> CommandError {
    let correlation_id = CorrelationId::generate();
    tracing::warn!(code = code.as_str(), %correlation_id, "security key operation failed");
    CommandError::new(code, message, correlation_id)
}
