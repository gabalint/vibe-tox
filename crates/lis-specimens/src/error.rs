//! The crate's error type. `Display` text is PHI-free and tells the user
//! what to do: it names fields, states and gates, never names, label content
//! or entered text.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

/// Failure of a specimens command or query.
#[derive(Debug, thiserror::Error)]
pub enum SpecimenError {
    /// An input field failed validation.
    #[error("Check the highlighted fields.")]
    Invalid(FieldError),
    /// The named record does not exist.
    #[error("The {0} was not found.")]
    NotFound(&'static str),
    /// The caller's expected revision is stale.
    #[error("The record changed since it was shown. Reload and try again.")]
    RevisionConflict,
    /// The record already exists with this code.
    #[error("A {0} with this code already exists.")]
    Duplicate(&'static str),
    /// The specimen's current acceptance does not allow this decision (REQ-SPC-002).
    #[error("This receipt decision is not allowed for the specimen's current acceptance.")]
    DecisionNotAllowed,
    /// The container's state does not allow this action.
    #[error("The container is {0}; this action is not allowed.")]
    ContainerState(&'static str),
    /// The specimen is held or rejected, so its material cannot be used.
    #[error("The specimen is {0}; its material cannot be used.")]
    SpecimenNotUsable(&'static str),
    /// An open hold blocks the action (REQ-SPC-005).
    #[error("The container has an open hold. Release the hold first.")]
    HoldOpen,
    /// The hold or excursion is already closed.
    #[error("The {0} is already closed.")]
    AlreadyClosed(&'static str),
    /// The aliquot would take more than the parent's known remaining volume (REQ-SPC-003).
    #[error("The volume taken exceeds the parent container's remaining volume.")]
    VolumeExceeded,
    /// A container that already has a label job needs a reprint with a reason (REQ-SPC-001).
    #[error("A label was already requested for this container. Request a reprint with a reason.")]
    AlreadyPrinted,
    /// The location or printer is inactive.
    #[error("The {0} is inactive.")]
    Inactive(&'static str),
    /// The label job already has an outcome.
    #[error("The label job outcome is already recorded.")]
    OutcomeRecorded,
    /// The principal kind is not allowed to perform this specimen operation.
    #[error("This operation is not allowed for this type of actor.")]
    ActorNotAllowed,
    /// The numbering sequence for this period is used up.
    #[error("The {0} number sequence is exhausted. Contact the laboratory administrator.")]
    NumberingExhausted(&'static str),
    /// An order item is not accepted, or its order has an open
    /// clarification hold (REQ-ORD-001).
    #[error(
        "An order item is not ready for accessioning: the order or item is not accepted, or the order is on hold."
    )]
    OrderItemNotReady,
    /// The order item is already linked to an accession.
    #[error("An order item is already accessioned.")]
    ItemAccessioned,
    /// An order lookup failed.
    #[error(transparent)]
    Orders(#[from] lis_orders::OrderError),
    /// A subject lookup failed.
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    /// The store failed or refused authorization.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl SpecimenError {
    /// The `CommandError` code the engine reports for this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) | Self::VolumeExceeded => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::ActorNotAllowed => ErrorCode::Forbidden,
            Self::RevisionConflict
            | Self::Duplicate(_)
            | Self::DecisionNotAllowed
            | Self::ContainerState(_)
            | Self::AlreadyClosed(_)
            | Self::AlreadyPrinted
            | Self::Inactive(_)
            | Self::OutcomeRecorded
            | Self::NumberingExhausted(_)
            | Self::ItemAccessioned => ErrorCode::Conflict,
            Self::SpecimenNotUsable(_) | Self::HoldOpen | Self::OrderItemNotReady => {
                ErrorCode::GateBlocked
            }
            Self::Orders(error) => error.code(),
            Self::Identity(error) => error.code(),
            Self::Store(error) => error.code(),
        }
    }

    /// The field errors of a validation failure.
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        let Self::Invalid(error) = self else {
            return Vec::new();
        };
        vec![error.clone()]
    }
}

impl From<rusqlite::Error> for SpecimenError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

/// A validation error for `field` with a stable code and a PHI-free message.
pub(crate) fn invalid(field: &str, code: &str, message: &str) -> SpecimenError {
    SpecimenError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}

/// Refuses a stale `expected` revision.
pub(crate) fn ensure_revision(current: i64, expected: i64) -> Result<(), SpecimenError> {
    if current == expected {
        Ok(())
    } else {
        Err(SpecimenError::RevisionConflict)
    }
}
