//! The crate's error type. `Display` text is PHI-free: it names fields,
//! records and opaque IDs, never names, birth dates or identifier values.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

use crate::model::SubjectId;

/// Failure of an identity command or query.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// Stored financial data could not be read without exposing its content.
    #[error("invalid financial record")]
    InvalidFinancialRecord,
    /// An input field failed validation.
    #[error("invalid {}: {}", .0.field, .0.code)]
    Invalid(FieldError),
    /// The named record does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// The caller's expected revision is stale.
    #[error("the record changed since it was read")]
    RevisionConflict,
    /// Identity corrections and hold decisions require a signed-in person.
    #[error("identity management needs a signed-in user")]
    UserRequired,
    /// The identifier is active, or historically belonged to another subject
    /// within its authority and type (INV-02).
    #[error("the identifier is already owned by subject {0}")]
    IdentifierInUse(SubjectId),
    /// Name and birth date candidates exist and the request did not
    /// acknowledge exactly the current set.
    #[error("possible duplicate subjects must be acknowledged")]
    CandidatesNotAcknowledged,
    /// The identity hold is no longer open.
    #[error("the identity hold is already resolved")]
    HoldResolved,
    /// The identifier is already inactive.
    #[error("the identifier is already inactive")]
    IdentifierInactive,
    /// A directory correction repeats every current value, so it would record no change.
    #[error("Nothing was changed. Change at least one value to save a correction.")]
    NothingChanged,
    /// A record of this kind already uses the code or NPI.
    #[error("a {0} with this code already exists")]
    Duplicate(&'static str),
    /// The store failed or refused authorization.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl IdentityError {
    /// The `CommandError` code the engine reports for this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidFinancialRecord => ErrorCode::Internal,
            Self::Invalid(_) | Self::NothingChanged => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::UserRequired => ErrorCode::Forbidden,
            Self::RevisionConflict
            | Self::IdentifierInUse(_)
            | Self::CandidatesNotAcknowledged
            | Self::HoldResolved
            | Self::IdentifierInactive
            | Self::Duplicate(_) => ErrorCode::Conflict,
            Self::Store(error) => error.code(),
        }
    }
}

impl From<rusqlite::Error> for IdentityError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

/// A validation error for `field` with a stable code and a PHI-free message.
pub(crate) fn invalid(field: &str, code: &str, message: &str) -> IdentityError {
    IdentityError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}

/// Refuses a stale `expected` revision.
pub(crate) fn ensure_revision(current: i64, expected: i64) -> Result<(), IdentityError> {
    if current == expected {
        Ok(())
    } else {
        Err(IdentityError::RevisionConflict)
    }
}
