//! Typed billing failures with PHI-free messages.

use lis_auth::SelfApprovalRefused;
use lis_core::{ErrorCode, FieldError, GateReason};
use lis_store::StoreError;

/// Failure of billing configuration, derivation or handoff work.
#[derive(Debug, thiserror::Error)]
pub enum BillingError {
    /// An untrusted field failed validation.
    #[error("invalid {}: {}", .0.field, .0.code)]
    Invalid(FieldError),
    /// The named record does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// The expected optimistic revision is stale.
    #[error("the billing record changed since it was read")]
    RevisionConflict,
    /// A mapping draft is already open.
    #[error("a billing class mapping draft is already open")]
    DraftExists,
    /// Only the author may edit a draft.
    #[error("only the author may edit this billing class mapping draft")]
    NotDraftAuthor,
    /// The approver wrote the mapping draft, and the laboratory's
    /// separation-of-duties policy requires a second person (ADR 0015).
    #[error(
        "you wrote this billing class mapping draft, and the laboratory's separation-of-duties \
         policy requires someone else to approve it; a quality manager can change that policy"
    )]
    SelfApproval,
    /// The mapping state does not permit this transition.
    #[error("the billing class mapping is not in the required state")]
    InvalidMappingState,
    /// A mapping needs billing approval before laboratory confirmation.
    #[error("the mapping needs billing approval before laboratory confirmation")]
    ApprovalRequired,
    /// Configuration changes require a person.
    #[error("billing configuration changes need a signed-in user")]
    UserRequired,
    /// A required analyte does not exist.
    #[error("a mapped analyte does not exist")]
    AnalyteNotFound,
    /// A blocking flag holds the derivation; the reasons name each flag.
    #[error("the derivation is held until its blocking flags are resolved")]
    DerivationHeld(Vec<GateReason>),
    /// The clinical inputs changed since the derivation was made.
    #[error("the derivation is out of date; derive the services again")]
    DerivationStale,
    /// The derivation's state does not permit this action.
    #[error("the derivation is not in the required state")]
    InvalidDerivationState,
    /// A derivation can be handed off once.
    #[error("the derivation was already handed off")]
    AlreadyHandedOff,
    /// The handoff content changed between building the file and recording it.
    #[error("the billing data changed while the handoff was built; try again")]
    HandoffChanged,
    /// The laboratory profile has no billing NPI or TIN.
    #[error("the laboratory billing NPI or TIN is missing")]
    LaboratoryBillingIdentityMissing,
    /// A handoff item has one outcome.
    #[error("an outcome is already recorded for this handoff item")]
    OutcomeRecorded,
    /// The stored handoff file is missing or damaged.
    #[error("the stored billing handoff file is missing or damaged")]
    HandoffFileUnavailable,
    /// Stored data contradicts an invariant.
    #[error("inconsistent stored {0}")]
    Inconsistent(&'static str),
    /// The store failed or refused authorization.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Catalog reference data could not be read.
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
    /// An identity lookup failed.
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    /// An order lookup failed.
    #[error(transparent)]
    Orders(#[from] lis_orders::OrderError),
    /// A specimen lookup failed.
    #[error(transparent)]
    Specimens(#[from] lis_specimens::SpecimenError),
    /// An execution lookup failed.
    #[error(transparent)]
    Analytical(#[from] lis_analytical::AnalyticalError),
    /// A result lookup failed.
    #[error(transparent)]
    Results(#[from] lis_results::ResultError),
    /// A report lookup failed.
    #[error(transparent)]
    Reporting(#[from] lis_reporting::ReportingError),
}

impl BillingError {
    /// Stable engine error code for this failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) | Self::AnalyteNotFound => ErrorCode::NotFound,
            Self::NotDraftAuthor | Self::UserRequired => ErrorCode::Forbidden,
            // ADR 0015: the separation-of-duties policy blocks it, not a missing capability.
            Self::SelfApproval => ErrorCode::GateBlocked,
            Self::RevisionConflict
            | Self::DraftExists
            | Self::InvalidMappingState
            | Self::ApprovalRequired
            | Self::DerivationStale
            | Self::InvalidDerivationState
            | Self::AlreadyHandedOff
            | Self::HandoffChanged
            | Self::OutcomeRecorded
            | Self::HandoffFileUnavailable => ErrorCode::Conflict,
            Self::DerivationHeld(_) | Self::LaboratoryBillingIdentityMissing => {
                ErrorCode::GateBlocked
            }
            Self::Inconsistent(_) => ErrorCode::Internal,
            Self::Store(error) => error.code(),
            Self::Catalog(error) => error.code(),
            Self::Identity(error) => error.code(),
            Self::Orders(error) => error.code(),
            Self::Specimens(error) => error.code(),
            Self::Analytical(error) => error.code(),
            Self::Results(error) => error.code(),
            Self::Reporting(error) => error.code(),
        }
    }

    /// Why a gate refused the action, for the UI's list.
    #[must_use]
    pub fn gate_reasons(&self) -> Vec<GateReason> {
        if let Self::DerivationHeld(reasons) = self {
            reasons.clone()
        } else {
            Vec::new()
        }
    }
}

impl From<rusqlite::Error> for BillingError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

/// ADR 0015: a policy that needs a second person keeps the author's refusal;
/// a missing confirmation is the `selfApproval` field error.
impl From<SelfApprovalRefused> for BillingError {
    fn from(refused: SelfApprovalRefused) -> Self {
        match refused {
            SelfApprovalRefused::NotAllowed(_) => Self::SelfApproval,
            SelfApprovalRefused::NotConfirmed(duty) => {
                Self::Invalid(duty.confirmation_field_error())
            }
        }
    }
}

impl From<lis_audit::AuditError> for BillingError {
    fn from(error: lis_audit::AuditError) -> Self {
        Self::Store(error.into())
    }
}

pub(crate) fn invalid(error: FieldError) -> BillingError {
    BillingError::Invalid(error)
}
