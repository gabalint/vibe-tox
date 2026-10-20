//! Typed failures of the runtime area: settings, app runs, health attestations, update
//! records and disposals. Messages are PHI-free and never echo input.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

/// Failure of a runtime-area command or query.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Input fields break a rule; see [`RuntimeError::field_errors`].
    #[error("Check the highlighted fields.")]
    Invalid(Vec<FieldError>),
    /// A named record does not exist.
    #[error("The {0} was not found.")]
    NotFound(&'static str),
    /// The expected settings version is not the one in force.
    #[error("Someone else changed the operations settings. Reload and try again.")]
    SettingsVersionConflict,
    /// A stale expected revision.
    #[error("The {0} changed since it was shown. Reload and try again.")]
    RevisionConflict(&'static str),
    /// The app run has already stopped.
    #[error("This app run has already stopped.")]
    RunStopped,
    /// The run has no listening gap longer than the limit.
    #[error("This app run has no listening gap to acknowledge.")]
    NoListeningGap,
    /// The gap was already acknowledged.
    #[error("This listening gap has already been acknowledged.")]
    GapAlreadyAcknowledged,
    /// The version already has an approval that is not withdrawn.
    #[error("This version is already approved.")]
    VersionAlreadyApproved,
    /// The backup is missing, not a retained verified manual/pre-update backup, or too old
    /// (REQ-OPS-006).
    #[error(
        "Choose a manual or pre-update backup with a retained verified copy, finished in the last 24 hours."
    )]
    PreUpdateBackupInvalid,
    /// The approval was already withdrawn.
    #[error("This update approval has already been withdrawn.")]
    ApprovalWithdrawn,
    /// The approved version has started, so the approval is part of its history.
    #[error("This version is already installed; its approval can no longer be withdrawn.")]
    ApprovalInstalled,
    /// Only the running version can be accepted.
    #[error("Only the version that is running now can be accepted.")]
    VersionNotRunning,
    /// The running version started without an approval and is not the first version.
    #[error("This version has no update approval. Record the approval first.")]
    VersionNotApproved,
    /// The running version was approved only after it started; acceptance needs a start
    /// under the approval (NFR-24).
    #[error(
        "This version was approved after it started. Restart the app so it starts under the approval, then accept it."
    )]
    VersionNotStartedSinceApproval,
    /// The version was already accepted for clinical use.
    #[error("This version has already been accepted.")]
    VersionAlreadyAccepted,
    /// Database, audit or authorization failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for RuntimeError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<FieldError> for RuntimeError {
    fn from(error: FieldError) -> Self {
        Self::Invalid(vec![error])
    }
}

impl RuntimeError {
    /// The `CommandError` code the engine reports.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::SettingsVersionConflict
            | Self::RevisionConflict(_)
            | Self::RunStopped
            | Self::NoListeningGap
            | Self::GapAlreadyAcknowledged
            | Self::VersionAlreadyApproved
            | Self::PreUpdateBackupInvalid
            | Self::ApprovalWithdrawn
            | Self::ApprovalInstalled
            | Self::VersionNotRunning
            | Self::VersionNotApproved
            | Self::VersionNotStartedSinceApproval
            | Self::VersionAlreadyAccepted => ErrorCode::Conflict,
            Self::Store(error) => error.code(),
        }
    }

    /// The field errors of [`RuntimeError::Invalid`]; empty otherwise.
    #[must_use]
    pub fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}
