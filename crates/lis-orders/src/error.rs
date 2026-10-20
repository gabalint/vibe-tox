//! Typed order failures. Display text contains no clinical content.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

/// Failure of an order command or query.
#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    /// A stored financial snapshot is invalid; content is never included in errors.
    #[error("invalid order billing record")]
    InvalidBillingRecord,
    /// Invalid input.
    #[error("invalid {}: {}", .0.field, .0.code)]
    Invalid(FieldError),
    /// Record not found.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// Optimistic concurrency check failed.
    #[error("the record changed since it was read")]
    RevisionConflict,
    /// Current state refuses the transition.
    #[error("the requested order transition is not allowed")]
    InvalidTransition,
    /// A safety gate blocks the action.
    #[error("an order gate blocks this action")]
    GateBlocked,
    /// A requested item predates the explicit sequence-role requirement.
    #[error("a legacy order item needs an explicit sequence role before acceptance")]
    LegacySequenceRoleMissing,
    /// A replay conflicts with an earlier decision.
    #[error("the trigger already has a different decision")]
    ReplayConflict,
    /// Database, audit or authorization failure.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Identity lookup failed.
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    /// Catalog lookup failed.
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
}

impl OrderError {
    /// Stable UI error classification.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidBillingRecord => ErrorCode::Internal,
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::RevisionConflict | Self::InvalidTransition | Self::ReplayConflict => {
                ErrorCode::Conflict
            }
            Self::GateBlocked | Self::LegacySequenceRoleMissing => ErrorCode::GateBlocked,
            Self::Store(error) => error.code(),
            Self::Identity(error) => error.code(),
            Self::Catalog(error) => error.code(),
        }
    }

    /// Structured input errors, empty for non-validation failures.
    #[must_use]
    pub fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(error) | Self::Identity(lis_identity::IdentityError::Invalid(error)) => {
                std::slice::from_ref(error)
            }
            Self::NotFound(_)
            | Self::InvalidBillingRecord
            | Self::RevisionConflict
            | Self::InvalidTransition
            | Self::GateBlocked
            | Self::LegacySequenceRoleMissing
            | Self::ReplayConflict
            | Self::Store(_)
            | Self::Identity(_)
            | Self::Catalog(_) => &[],
        }
    }
}

impl From<rusqlite::Error> for OrderError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

pub(crate) fn invalid(field: &str, code: &str, message: &str) -> OrderError {
    OrderError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}
