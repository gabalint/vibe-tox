//! Typed failures of the anchors area. Messages are PHI-free and never name a path.

use lis_core::ErrorCode;
use lis_store::StoreError;

use crate::backup::BackupError;

/// Failure of an anchor export, status query or verification.
#[derive(Debug, thiserror::Error)]
pub enum AnchorError {
    /// The installation row is missing; the engine writes it at the first start.
    #[error("the installation is not initialized")]
    NoInstallation,
    /// A manual export found no available target.
    #[error("Connect a backup target first: none is available.")]
    NoTargetAvailable,
    /// The chosen folder could not be read.
    #[error("The chosen folder cannot be read.")]
    FolderUnreadable,
    /// Reading the targets failed.
    #[error(transparent)]
    Targets(#[from] BackupError),
    /// Database, audit or authorization failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for AnchorError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<lis_audit::AuditError> for AnchorError {
    fn from(error: lis_audit::AuditError) -> Self {
        Self::Store(error.into())
    }
}

impl AnchorError {
    /// Stable UI error classification.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NoTargetAvailable | Self::FolderUnreadable => ErrorCode::Conflict,
            Self::NoInstallation => ErrorCode::Internal,
            Self::Targets(error) => error.code(),
            Self::Store(error) => error.code(),
        }
    }
}
