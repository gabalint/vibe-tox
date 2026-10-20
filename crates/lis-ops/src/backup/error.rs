//! Typed failures of the backup area: targets, runs, the recovery kit, restore, the
//! recovery reconciliation and the exit export. Messages are PHI-free and never name a
//! path or key material.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

use crate::format::FormatError;

/// Failure of a backup-area command, query or run.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// Invalid command input.
    #[error("Check the highlighted fields.")]
    Invalid(FieldError),
    /// A named record does not exist.
    #[error("The {0} was not found.")]
    NotFound(&'static str),
    /// A stale expected revision.
    #[error("The {0} changed since it was shown. Reload and try again.")]
    RevisionConflict(&'static str),
    /// The installation row is missing; the engine writes it at the first start.
    #[error("the installation is not initialized")]
    NoInstallation,
    /// The chosen folder does not exist, is not a folder or has an unsupported name.
    #[error("The chosen folder cannot be used as a backup target.")]
    TargetFolderUnusable,
    /// The chosen folder is inside the application's data folder, so it would be lost with it.
    #[error("Choose a folder outside the application's data folder.")]
    TargetInsideDataFolder,
    /// The marker could not be written to the chosen folder.
    #[error("The chosen folder is not writable.")]
    TargetNotWritable,
    /// The folder holds the backups of another installation.
    #[error("The chosen folder holds backups of another installation.")]
    TargetOfAnotherInstallation,
    /// The folder holds backups written under another installation key.
    #[error("The chosen folder holds backups written under another installation key.")]
    TargetKeyMismatch,
    /// The folder is already an active target.
    #[error("The chosen folder is already a backup target.")]
    TargetAlreadyAdded,
    /// The target is retired.
    #[error("This backup target is retired.")]
    TargetRetired,
    /// No active target takes backups.
    #[error("Add a backup target first.")]
    NoBackupTarget,
    /// A manual or pre-update backup is already waiting.
    #[error("A backup request is already waiting.")]
    RequestQueued,
    /// Only the latest recovery kit can be confirmed.
    #[error("Only the latest recovery kit can be confirmed as stored.")]
    KitNotLatest,
    /// The installation key text is not a `vtk1-` key.
    #[error("This is not an installation key from a recovery kit.")]
    KeyFormat,
    /// The installation key text has a wrong checksum: a typing error.
    #[error("The installation key has a typing error; check each group.")]
    KeyChecksum,
    /// The recovery kit file could not be written.
    #[error("The recovery kit file could not be written.")]
    KitNotWritten,
    /// A restore test is already waiting.
    #[error("A restore test is already waiting.")]
    RestoreTestQueued,
    /// Restoring needs a new installation: user accounts exist (M4 design brief D4).
    #[error("Restore is only possible on a new installation without user accounts.")]
    InstallationInUse,
    /// The restore session ended; the key must be entered again.
    #[error("The restore session expired. Enter the installation key again.")]
    RestoreSessionExpired,
    /// The chosen folder holds no backups of this application.
    #[error("The chosen folder holds no backups of this application.")]
    NoBackupsInFolder,
    /// The set belongs to an installation of the other kind: a demo backup is never
    /// restored into a laboratory installation, nor the reverse (proposed REQ-OPS-016).
    #[error(
        "This backup is from a {backup} installation and cannot be restored into a {installation} installation."
    )]
    InstallationKindMismatch {
        /// The kind of the installation that made the backup.
        backup: &'static str,
        /// The kind of this installation.
        installation: &'static str,
    },
    /// The staged set did not pass every check, or nothing is staged.
    #[error("The staged backup did not pass every check; it cannot be restored.")]
    RestoreNotVerified,
    /// No recovery reconciliation is open for this restore.
    #[error("This recovery reconciliation is not open.")]
    RecoveryNotOpen,
    /// An exit export is already waiting or running.
    #[error("An exit export is already running.")]
    ExitExportRunning,
    /// The folder chosen for an exit export does not exist or is inside the data folder.
    #[error(
        "The chosen folder cannot hold the exit export. Choose an existing folder outside the application's data folder."
    )]
    ExportFolderUnusable,
    /// An encrypted file could not be written or read.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A file system operation outside the format failed.
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
    /// Database, audit or authorization failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for BackupError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<lis_audit::AuditError> for BackupError {
    fn from(error: lis_audit::AuditError) -> Self {
        Self::Store(error.into())
    }
}

impl BackupError {
    /// Stable UI error classification.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) | Self::KeyFormat | Self::KeyChecksum => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::RevisionConflict(_)
            | Self::TargetFolderUnusable
            | Self::TargetInsideDataFolder
            | Self::TargetNotWritable
            | Self::TargetOfAnotherInstallation
            | Self::TargetKeyMismatch
            | Self::TargetAlreadyAdded
            | Self::TargetRetired
            | Self::NoBackupTarget
            | Self::RequestQueued
            | Self::KitNotLatest
            | Self::KitNotWritten
            | Self::RestoreTestQueued
            | Self::InstallationInUse
            | Self::RestoreSessionExpired
            | Self::NoBackupsInFolder
            | Self::InstallationKindMismatch { .. }
            | Self::RestoreNotVerified
            | Self::RecoveryNotOpen
            | Self::ExitExportRunning
            | Self::ExportFolderUnusable => ErrorCode::Conflict,
            Self::NoInstallation | Self::Format(_) | Self::Io(_) => ErrorCode::Internal,
            Self::Store(error) => error.code(),
        }
    }
}
