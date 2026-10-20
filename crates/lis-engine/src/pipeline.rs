//! The command and query pipeline (`.ai/skills/engine-command/SKILL.md`):
//! caller resolution, writes on the single writer, reads on read connections,
//! audited reads, denial events, error mapping and the one completion log
//! line per command.

use std::future::Future;
use std::time::{Duration, Instant};

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::{Attempt, AuthError};
use lis_core::{CommandError, CorrelationId, Denied, ErrorCode, FieldError, GateReason, Principal};
use lis_store::{StoreError, WriteCommand};
use rusqlite::Connection;

use crate::engine::Engine;
use crate::registry::{Access, CommandSpec};

/// The system principal for commands run at the sign-in screen, before anyone is signed in.
pub(crate) const SIGN_IN_SCREEN: &str = "sign-in-screen";

/// Why a command did not succeed, before it becomes a [`CommandError`].
#[derive(Debug)]
pub(crate) enum Failure {
    /// A typed failure from an existing command or trusted native adapter.
    Forwarded(CommandError),
    /// An expected refusal with a safe, PHI-free message.
    Refused {
        code: ErrorCode,
        message: String,
        field_errors: Vec<FieldError>,
        /// Why a gate refused the action, shown to the user as a list.
        gate_reasons: Vec<GateReason>,
    },
    /// Authorization refused the command; the denial is already audited.
    Denied,
    /// An unexpected failure. The detail is logged, never shown.
    Internal(String),
}

impl Failure {
    pub(crate) fn refused(code: ErrorCode, message: &str) -> Self {
        Self::Refused {
            code,
            message: message.to_owned(),
            field_errors: Vec::new(),
            gate_reasons: Vec::new(),
        }
    }

    fn into_command_error(self, correlation_id: CorrelationId) -> CommandError {
        match self {
            Self::Forwarded(error) => error,
            Self::Refused {
                code,
                message,
                field_errors,
                gate_reasons,
            } => CommandError::new(code, message, correlation_id)
                .with_field_errors(field_errors)
                .with_gate_reasons(gate_reasons),
            Self::Denied => CommandError::new(
                ErrorCode::Forbidden,
                "You do not have permission for this action.",
                correlation_id,
            ),
            Self::Internal(_) => CommandError::new(
                ErrorCode::Internal,
                "The app could not complete the action. Give support the reference shown.",
                correlation_id,
            ),
        }
    }
}

impl From<AuthError> for Failure {
    fn from(error: AuthError) -> Self {
        match error.code() {
            ErrorCode::Internal => Self::Internal(error.to_string()),
            code => Self::Refused {
                code,
                message: error.to_string(),
                field_errors: error.field_errors().to_vec(),
                gate_reasons: Vec::new(),
            },
        }
    }
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        AuthError::Store(error).into()
    }
}

/// The accepted value of an attempt, or the refusal as a failure.
pub(crate) fn accepted<T>(attempt: Attempt<T>) -> Result<T, Failure> {
    match attempt {
        Attempt::Accepted(value) => Ok(value),
        Attempt::Rejected(rejection) => Err(Failure::Refused {
            code: rejection.code(),
            message: rejection.to_string(),
            field_errors: rejection.field_errors(),
            gate_reasons: Vec::new(),
        }),
    }
}

/// A module error the pipeline can classify.
pub(crate) trait ModuleError: From<StoreError> + Into<Failure> + Send + 'static {
    /// The authorization denial inside, if that is what this is.
    fn denied(&self) -> Option<Denied>;
}

impl ModuleError for StoreError {
    fn denied(&self) -> Option<Denied> {
        match self {
            Self::Denied(denied) => Some(*denied),
            _ => None,
        }
    }
}

impl ModuleError for AuthError {
    fn denied(&self) -> Option<Denied> {
        match self {
            Self::Store(error) => error.denied(),
            _ => None,
        }
    }
}

/// One command invocation: its registry entry, correlation ID and caller.
pub(crate) struct Call {
    pub(crate) spec: &'static CommandSpec,
    pub(crate) correlation_id: CorrelationId,
    pub(crate) principal: Principal,
}

impl Engine {
    /// Runs one IPC command: resolves the caller for `spec.access`, runs
    /// `body`, logs one completion line and maps the failure to a [`CommandError`].
    pub(crate) async fn run<T, F, Fut>(
        &self,
        spec: &'static CommandSpec,
        body: F,
    ) -> Result<T, CommandError>
    where
        F: FnOnce(Call) -> Fut,
        Fut: Future<Output = Result<T, Failure>>,
    {
        let started = Instant::now();
        let correlation_id = CorrelationId::generate();
        let (principal, result) = match self.caller(spec.access, correlation_id).await {
            Ok(principal) => {
                let label = principal.actor().id_text();
                let call = Call {
                    spec,
                    correlation_id,
                    principal,
                };
                (label, body(call).await)
            }
            Err(failure) => ("none".to_owned(), Err(failure)),
        };
        log_completion(spec, correlation_id, &principal, &result, started.elapsed());
        result.map_err(|failure| failure.into_command_error(correlation_id))
    }

    async fn caller(
        &self,
        access: Access,
        correlation_id: CorrelationId,
    ) -> Result<Principal, Failure> {
        match access {
            Access::Public => Ok(Principal::system(SIGN_IN_SCREEN, [])),
            Access::AnySession
            | Access::Session
            | Access::Capability(_)
            | Access::EitherCapability(_, _) => {
                self.session_principal(access, correlation_id).await
            }
        }
    }

    /// Runs `command` in a write transaction as the caller. A denial rolls
    /// back and is then audited in its own transaction.
    pub(crate) async fn write<T, E, F>(&self, call: &Call, command: F) -> Result<T, Failure>
    where
        F: WriteCommand<T, E>,
        T: Send + 'static,
        E: ModuleError,
    {
        match self
            .db
            .write(call.principal.clone(), call.correlation_id, command)
            .await
        {
            Ok(value) => Ok(value),
            Err(error) => match error.denied() {
                Some(denied) => Err(self.record_denial(call, denied).await),
                None => Err(error.into()),
            },
        }
    }

    /// Runs `query` on a read connection after the registry's capability check.
    pub(crate) async fn read<T, E, F>(&self, call: &Call, query: F) -> Result<T, Failure>
    where
        F: FnOnce(&Connection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: ModuleError,
    {
        self.authorize(call).await?;
        self.db.read(query).await.map_err(Into::into)
    }

    /// A sensitive read: the `access` audit event `event(&data)` is appended
    /// before the data is returned, and the read fails if it cannot be.
    pub(crate) async fn audited_read<T, E, F, A>(
        &self,
        call: &Call,
        query: F,
        event: A,
    ) -> Result<T, Failure>
    where
        F: FnOnce(&Connection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: ModuleError,
        A: FnOnce(&T) -> AuditEvent,
    {
        let data = self.read(call, query).await?;
        let event = event(&data);
        let access = call.spec.access;
        self.write(call, move |ctx| {
            if let Some(capability) = access.require(ctx.principal()).map_err(StoreError::from)? {
                ctx.require(capability)?;
            }
            ctx.audit(event).map(drop)
        })
        .await
        .map(|()| data)
    }

    /// The registry's capability check for paths without a domain `require`.
    pub(crate) async fn authorize(&self, call: &Call) -> Result<(), Failure> {
        if let Err(denied) = call.spec.access.require(&call.principal) {
            return Err(self.record_denial(call, denied).await);
        }
        Ok(())
    }

    /// Records a denial as a `security` event with outcome `denied`.
    async fn record_denial(&self, call: &Call, denied: Denied) -> Failure {
        let event = AuditEvent::new(Stream::Security, call.spec.action, Outcome::Denied)
            .detail("capability", AuditDetail::code(denied.capability.as_str()));
        let recorded: Result<(), StoreError> = self
            .db
            .write(call.principal.clone(), call.correlation_id, move |ctx| {
                ctx.audit(event).map(drop)
            })
            .await;
        match recorded {
            Ok(()) => Failure::Denied,
            Err(error) => Failure::Internal(format!("denial not audited: {error}")),
        }
    }
}

/// The principal ID for log lines: a user or instrument ID, or a component name.
/// The one completion line per command: no input, output or message text.
fn log_completion<T>(
    spec: &CommandSpec,
    correlation_id: CorrelationId,
    principal: &str,
    result: &Result<T, Failure>,
    elapsed: Duration,
) {
    let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let (outcome, error_code) = match result {
        Ok(_) => ("succeeded", None),
        Err(Failure::Forwarded(error)) => ("failed", Some(error.code().as_str())),
        Err(Failure::Refused { code, .. }) => ("failed", Some(code.as_str())),
        Err(Failure::Denied) => ("denied", Some(ErrorCode::Forbidden.as_str())),
        Err(Failure::Internal(detail)) => {
            tracing::error!(
                command = spec.name,
                action = spec.action,
                %correlation_id,
                principal,
                outcome = "failed",
                error_code = ErrorCode::Internal.as_str(),
                error = %detail,
                duration_ms,
                "command completed"
            );
            return;
        }
    };
    tracing::info!(
        command = spec.name,
        action = spec.action,
        %correlation_id,
        principal,
        outcome,
        error_code,
        duration_ms,
        "command completed"
    );
}

/// The failure for a domain error's code, PHI-free text, field errors and gate
/// reasons: internal errors are logged and never shown; field errors come with a
/// prompt to check them.
pub(crate) fn domain_failure(
    code: ErrorCode,
    message: String,
    field_errors: Vec<FieldError>,
    gate_reasons: Vec<GateReason>,
) -> Failure {
    match code {
        ErrorCode::Internal => Failure::Internal(message),
        code if !field_errors.is_empty() => Failure::Refused {
            code,
            message: "Check the highlighted fields.".to_owned(),
            field_errors,
            gate_reasons,
        },
        code => Failure::Refused {
            code,
            message,
            field_errors,
            gate_reasons,
        },
    }
}

/// Implements [`ModuleError`] and the conversion to [`Failure`] for a domain
/// error enum with a `Store(StoreError)` variant and a `code()` method.
/// `$field_errors` names the input fields at fault, which the UI highlights;
/// the optional `$gate_reasons` lists why a gate refused the action.
macro_rules! domain_error {
    ($error:ty, $field_errors:expr) => {
        $crate::pipeline::domain_error!($error, $field_errors, |_: &$error| Vec::new());
    };
    ($error:ty, $field_errors:expr, $gate_reasons:expr) => {
        impl $crate::pipeline::ModuleError for $error {
            fn denied(&self) -> Option<lis_core::Denied> {
                if let Self::Store(error) = self {
                    error.denied()
                } else {
                    None
                }
            }
        }

        impl From<$error> for $crate::pipeline::Failure {
            fn from(error: $error) -> Self {
                let field_errors: Vec<lis_core::FieldError> = $field_errors(&error);
                let gate_reasons: Vec<lis_core::GateReason> = $gate_reasons(&error);
                $crate::pipeline::domain_failure(
                    error.code(),
                    error.to_string(),
                    field_errors,
                    gate_reasons,
                )
            }
        }
    };
}
pub(crate) use domain_error;

/// The `access` event of a sensitive read; the read fails if it cannot be appended.
pub(crate) fn access(action: &'static str) -> AuditEvent {
    AuditEvent::new(Stream::Access, action, Outcome::Succeeded)
}
