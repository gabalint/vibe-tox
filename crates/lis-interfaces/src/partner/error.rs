use lis_core::{ErrorCode, FieldError, GateReason};
use lis_store::StoreError;

/// Failures of connection, routing and delivery commands and queries.
#[derive(Debug, thiserror::Error)]
pub enum PartnerError {
    /// The store, audit trail or authorization check failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// An input field is not valid; the field error says which and why.
    #[error("{}", .0.message)]
    Invalid(FieldError),
    /// The record does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// The connection changed since it was shown.
    #[error("This connection was changed by someone else. Reload it and try again.")]
    RevisionConflict,
    /// The action does not apply to the record as it is now.
    #[error("{0}")]
    WrongState(&'static str),
    /// A gate refused the action; the reasons say why.
    #[error("The action is not allowed now.")]
    GateBlocked(Vec<GateReason>),
    /// Stored data does not have the expected shape.
    #[error("stored {0} is inconsistent")]
    Inconsistent(&'static str),
}

impl PartnerError {
    /// The `CommandError` code the engine reports for this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Store(error) => error.code(),
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::RevisionConflict | Self::WrongState(_) => ErrorCode::Conflict,
            Self::GateBlocked(_) => ErrorCode::GateBlocked,
            Self::Inconsistent(_) => ErrorCode::Internal,
        }
    }

    /// The input field at fault, for the UI to highlight.
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        match self {
            Self::Invalid(error) => vec![error.clone()],
            Self::Store(_)
            | Self::NotFound(_)
            | Self::RevisionConflict
            | Self::WrongState(_)
            | Self::GateBlocked(_)
            | Self::Inconsistent(_) => Vec::new(),
        }
    }

    /// Why a gate refused the action.
    #[must_use]
    pub fn gate_reasons(&self) -> Vec<GateReason> {
        match self {
            Self::GateBlocked(reasons) => reasons.clone(),
            Self::Store(_)
            | Self::Invalid(_)
            | Self::NotFound(_)
            | Self::RevisionConflict
            | Self::WrongState(_)
            | Self::Inconsistent(_) => Vec::new(),
        }
    }
}

impl From<rusqlite::Error> for PartnerError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<serde_json::Error> for PartnerError {
    fn from(_: serde_json::Error) -> Self {
        Self::Inconsistent("connection settings")
    }
}

/// A field error on `field`.
pub(crate) fn invalid(field: &str, code: &str, message: &str) -> PartnerError {
    PartnerError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}
