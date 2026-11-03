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
    /// A safety gate blocks the action; the message names the gate.
    #[error("{0}")]
    GateBlocked(OrderGate),
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

/// The order gate that refused an action, named in plain words for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OrderGate {
    /// The test version is not approved or has been retired.
    #[error("The test version is not approved or is no longer in use. Choose a current version.")]
    InactiveTestVersion,
    /// An open clarification hold blocks acceptance.
    #[error("The order has an open clarification hold. Resolve the hold first.")]
    OpenClarificationHold,
    /// An add-on is accepted only with the evidence that authorizes it.
    #[error(
        "This add-on has no authority reference. Decline it and record a new request with \
         the document or call that authorizes it."
    )]
    AddOnWithoutAuthority,
    /// The parent test is on a different order.
    #[error("The parent test belongs to a different order.")]
    ParentOnOtherOrder,
    /// A reflex needs the test that triggered it.
    #[error("A reflex needs the test that triggered it.")]
    ParentMissing,
    /// The parent test is not accepted.
    #[error("The parent test is not accepted.")]
    ParentNotAccepted,
    /// The order does not authorize reflex testing for the parent test.
    #[error("The order does not authorize reflex testing for the parent test.")]
    ReflexNotAuthorized,
    /// The test is outside the parent test's authorized reflex scope.
    #[error("This test is outside the reflex scope authorized for the parent test.")]
    OutsideReflexScope,
    /// The point-of-care result is not recorded on this order.
    #[error("The point-of-care result is not recorded on this order.")]
    PointOfCareResultMissing,
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
            Self::GateBlocked(_) | Self::LegacySequenceRoleMissing => ErrorCode::GateBlocked,
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
            | Self::GateBlocked(_)
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
