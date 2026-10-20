//! The crate's error type. `Display` text is PHI-free and tells the user
//! what to do: it names fields, states and gates, never labels, values or
//! entered text.

use lis_auth::SelfApprovalRefused;
use lis_core::{ErrorCode, FieldError, GateReason};
use lis_store::StoreError;

/// Failure of an analytical command or query.
#[derive(Debug, thiserror::Error)]
pub enum AnalyticalError {
    /// An input field failed validation.
    #[error("Check the highlighted fields.")]
    Invalid(FieldError),
    /// The named record does not exist.
    #[error("The {0} was not found.")]
    NotFound(&'static str),
    /// The caller's expected revision is stale.
    #[error("The record changed since it was shown. Reload and try again.")]
    RevisionConflict,
    /// The record already exists.
    #[error("This {0} already exists.")]
    Duplicate(&'static str),
    /// The record's state does not allow this action.
    #[error("The {0} is not in a state that allows this action.")]
    WrongState(&'static str),
    /// One or more safety gates block the action; each reason is PHI-free.
    #[error("{}", gate_text(.0))]
    GateBlocked(Vec<GateReason>),
    /// Approval and QC decisions need a signed-in person.
    #[error("A signed-in user must make this decision.")]
    UserRequired,
    /// The approver wrote the registration, and the laboratory's
    /// separation-of-duties policy requires a second person (ADR 0015).
    #[error(
        "You wrote this registration, and your laboratory's separation-of-duties policy requires someone else to approve it. A quality manager can change that policy."
    )]
    SameAuthor,
    /// The container already has an active execution for these components.
    #[error(
        "The material already has an active execution for these tests. Record a rerun, re-extraction or dilution with a reason."
    )]
    AlreadyInProgress,
    /// Another active execution already serves the order component (F17).
    #[error("Another active execution already performs this test.")]
    AlreadyServed,
    /// The run's method serves no testable component of this material.
    #[error("The run's method has no pending test for this material.")]
    NothingToTest,
    /// The QC data changed since the assessment was prepared.
    #[error("The QC data changed since it was shown. Reload and assess again.")]
    EvidenceChanged,
    /// A rejected QC assessment cannot be superseded by an acceptance (REQ-ANA-005).
    #[error(
        "A rejected QC assessment cannot be changed to accepted. Rerun the affected work instead."
    )]
    RejectionFinal,
    /// A laboratory profile lookup failed.
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    /// A catalog lookup failed.
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
    /// An order lookup failed.
    #[error(transparent)]
    Orders(#[from] lis_orders::OrderError),
    /// A specimen lookup failed.
    #[error(transparent)]
    Specimens(#[from] lis_specimens::SpecimenError),
    /// An instrument lookup failed.
    #[error(transparent)]
    Interfaces(#[from] lis_interfaces::InterfaceError),
    /// The store failed or refused authorization.
    #[error(transparent)]
    Store(#[from] StoreError),
}

fn gate_text(gates: &[GateReason]) -> String {
    let reasons: Vec<&str> = gates.iter().map(|gate| gate.message.as_str()).collect();
    format!("This action is blocked. {}", reasons.join(" "))
}

impl AnalyticalError {
    /// The `CommandError` code the engine reports for this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::RevisionConflict
            | Self::Duplicate(_)
            | Self::WrongState(_)
            | Self::AlreadyInProgress
            | Self::AlreadyServed
            | Self::NothingToTest
            | Self::EvidenceChanged
            | Self::RejectionFinal => ErrorCode::Conflict,
            // ADR 0015: the separation-of-duties policy blocks `SameAuthor`, not a capability.
            Self::GateBlocked(_) | Self::SameAuthor => ErrorCode::GateBlocked,
            Self::UserRequired => ErrorCode::Forbidden,
            Self::Identity(error) => error.code(),
            Self::Catalog(error) => error.code(),
            Self::Orders(error) => error.code(),
            Self::Specimens(error) => error.code(),
            Self::Interfaces(error) => error.code(),
            Self::Store(error) => error.code(),
        }
    }

    /// The field errors of a validation failure.
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        if let Self::Invalid(error) = self {
            vec![error.clone()]
        } else {
            Vec::new()
        }
    }

    /// The gate reasons of a blocked action.
    #[must_use]
    pub fn gate_reasons(&self) -> &[GateReason] {
        if let Self::GateBlocked(gates) = self {
            gates
        } else {
            &[]
        }
    }
}

impl From<rusqlite::Error> for AnalyticalError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<FieldError> for AnalyticalError {
    fn from(error: FieldError) -> Self {
        Self::Invalid(error)
    }
}

/// ADR 0015: a policy that needs a second person keeps the author's refusal;
/// a missing confirmation is the `selfApproval` field error.
impl From<SelfApprovalRefused> for AnalyticalError {
    fn from(refused: SelfApprovalRefused) -> Self {
        match refused {
            SelfApprovalRefused::NotAllowed(_) => Self::SameAuthor,
            SelfApprovalRefused::NotConfirmed(duty) => {
                Self::Invalid(duty.confirmation_field_error())
            }
        }
    }
}

/// A validation error for `field` with a stable code and a PHI-free message.
pub(crate) fn invalid(field: &str, code: &str, message: &str) -> AnalyticalError {
    AnalyticalError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}

/// A blocking gate with a stable code and a PHI-free message.
pub(crate) fn gate(code: &str, message: &str) -> GateReason {
    GateReason {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

/// Refuses a stale `expected` revision.
pub(crate) fn ensure_revision(current: i64, expected: i64) -> Result<(), AnalyticalError> {
    if current == expected {
        Ok(())
    } else {
        Err(AnalyticalError::RevisionConflict)
    }
}

/// Refuses an action when any gate blocks it.
pub(crate) fn ensure_clear(gates: Vec<GateReason>) -> Result<(), AnalyticalError> {
    if gates.is_empty() {
        Ok(())
    } else {
        Err(AnalyticalError::GateBlocked(gates))
    }
}
