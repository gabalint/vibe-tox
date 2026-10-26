//! Typed result failures. Display text is PHI-free and never echoes source data.

use lis_core::{ErrorCode, FieldError, GateReason};
use lis_store::StoreError;

/// Failure of a result import, command or query.
#[derive(Debug, thiserror::Error)]
pub enum ResultError {
    /// Invalid command input.
    #[error("Check the highlighted fields.")]
    Invalid(FieldError),
    /// A named record does not exist.
    #[error("The {0} was not found.")]
    NotFound(&'static str),
    /// Optimistic concurrency or current-revision check failed.
    #[error("The record changed since it was shown. Reload and try again.")]
    RevisionConflict,
    /// The record state refuses this action.
    #[error("The {0} is not in a state that allows this action.")]
    WrongState(&'static str),
    /// Every selected result revision already has a current approved review.
    #[error("These results are already approved. Nothing new to review.")]
    AlreadyApproved,
    /// One or more safety gates block approval.
    #[error("This action is blocked by one or more review gates.")]
    GateBlocked(Vec<GateReason>),
    /// A decision requiring attribution was attempted without a user.
    #[error("A signed-in user must make this decision.")]
    UserRequired,
    /// An upstream analytical operation failed.
    #[error(transparent)]
    Analytical(#[from] lis_analytical::AnalyticalError),
    /// A catalog lookup failed.
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
    /// An interface receipt lookup failed.
    #[error(transparent)]
    Interfaces(#[from] lis_interfaces::InterfaceError),
    /// An order lookup or follow-on command failed.
    #[error(transparent)]
    Orders(#[from] lis_orders::OrderError),
    /// A specimen lookup failed.
    #[error(transparent)]
    Specimens(#[from] lis_specimens::SpecimenError),
    /// Database, audit, outbox or authorization failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ResultError {
    /// Stable UI error classification.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::RevisionConflict | Self::WrongState(_) | Self::AlreadyApproved => {
                ErrorCode::Conflict
            }
            Self::GateBlocked(_) => ErrorCode::GateBlocked,
            Self::UserRequired => ErrorCode::Forbidden,
            Self::Analytical(error) => error.code(),
            Self::Catalog(error) => error.code(),
            Self::Interfaces(error) => error.code(),
            Self::Orders(error) => error.code(),
            Self::Specimens(error) => error.code(),
            Self::Store(error) => error.code(),
        }
    }

    /// Structured input errors, including those of the analytical and
    /// specimen steps a results command runs (`use_measured_result`).
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        match self {
            Self::Invalid(error) => vec![error.clone()],
            Self::Analytical(error) => error.field_errors(),
            Self::Specimens(error) => error.field_errors(),
            Self::NotFound(_)
            | Self::RevisionConflict
            | Self::WrongState(_)
            | Self::AlreadyApproved
            | Self::GateBlocked(_)
            | Self::UserRequired
            | Self::Catalog(_)
            | Self::Interfaces(_)
            | Self::Orders(_)
            | Self::Store(_) => Vec::new(),
        }
    }

    /// Blocking reasons, when this is a gate refusal, including those of the
    /// analytical step a results command runs (`use_measured_result`).
    #[must_use]
    pub fn gate_reasons(&self) -> &[GateReason] {
        match self {
            Self::GateBlocked(gates) => gates,
            Self::Analytical(error) => error.gate_reasons(),
            Self::Invalid(_)
            | Self::NotFound(_)
            | Self::RevisionConflict
            | Self::WrongState(_)
            | Self::AlreadyApproved
            | Self::UserRequired
            | Self::Catalog(_)
            | Self::Interfaces(_)
            | Self::Orders(_)
            | Self::Specimens(_)
            | Self::Store(_) => &[],
        }
    }
}

impl From<rusqlite::Error> for ResultError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

pub(crate) fn invalid(field: &str, code: &str, message: &str) -> ResultError {
    ResultError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}

pub(crate) fn gate(code: &str, message: &str) -> GateReason {
    GateReason {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

pub(crate) fn require_reason(field: &str, reason: &str) -> Result<(), ResultError> {
    lis_core::required_text(field, reason, 1000)
        .map(drop)
        .map_err(ResultError::Invalid)
}

pub(crate) fn require_user(ctx: &lis_store::CommandContext<'_>) -> Result<String, ResultError> {
    match ctx.principal().actor() {
        lis_core::Actor::User { user_id, .. } => Ok(user_id.to_string()),
        lis_core::Actor::Instrument { .. } | lis_core::Actor::System { .. } => {
            Err(ResultError::UserRequired)
        }
    }
}
