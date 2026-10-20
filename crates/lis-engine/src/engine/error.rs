//! Why the engine could not start, and the PHI-free codes the startup-failure screen shows
//! (M4 design brief §6.4).

use lis_auth::AuthError;
use lis_interfaces::InterfaceError;
use lis_ops::backup::BackupError;
use lis_ops::{InstallationError, RuntimeError};
use lis_store::{MigrationProblem, StoreError};

/// Why the engine could not start. [`EngineError::code`] is what the startup-failure
/// screen shows.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The data folder is on a network file system, where SQLite WAL fails (D15).
    #[error("the data folder is on a network file system")]
    DataFolderOnNetwork,
    /// The data folder's volume could not be classified as a local disk (D15): the app
    /// starts only on a volume it knows is local.
    #[error("the data folder's volume could not be classified as local")]
    DataFolderUnknown,
    /// A confirmed restore could not be put in place; it resumes at the next start.
    #[error("the staged restore could not be applied: {0}")]
    RestoreApply(BackupError),
    /// An applied restore could not be recorded: the recovery gate must never be missing.
    #[error("the applied restore could not be recorded: {0}")]
    RestoreRecord(String),
    /// The data folder holds an installation of the other kind: a demo database never opens
    /// as a laboratory's, nor the reverse (proposed REQ-OPS-016).
    #[error("the data folder holds a {found} installation, not a {expected} one")]
    InstallationKindMismatch {
        /// The kind the app was asked to open.
        expected: &'static str,
        /// The kind the database was created as.
        found: &'static str,
    },
    /// Opening or migrating the database failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A startup reconciliation step failed.
    #[error(transparent)]
    Auth(#[from] AuthError),
    /// Receipt reconciliation failed.
    #[error(transparent)]
    Interfaces(#[from] InterfaceError),
    /// Recording the app run or the interrupted jobs failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    /// Marking interrupted jobs failed.
    #[error(transparent)]
    Backup(#[from] BackupError),
    /// Reading what the installation key protects failed (the message names no key
    /// material).
    #[error("the installation key's state could not be decided: {0}")]
    InstallationKey(String),
    /// A blocking startup step stopped unexpectedly.
    #[error("a startup step stopped unexpectedly")]
    StartupTask,
}

impl EngineError {
    /// The PHI-free code the startup-failure screen explains (§6.4).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::DataFolderOnNetwork | Self::Store(StoreError::WalUnavailable) => {
                "data_folder_on_network"
            }
            Self::DataFolderUnknown => "data_folder_unknown",
            Self::Store(StoreError::AlreadyOpen) => "already_open",
            Self::Store(StoreError::Migration {
                problem: MigrationProblem::Missing,
                ..
            }) => "database_newer_than_app",
            Self::Store(
                StoreError::PreMigrationSnapshotNotIntact
                | StoreError::PreMigrationSnapshotFailed(_),
            ) => "pre_migration_snapshot_failed",
            Self::Store(StoreError::Sql(_) | StoreError::Migration { .. }) => "database_unreadable",
            Self::RestoreApply(_) => "restore_apply_failed",
            Self::InstallationKindMismatch { .. } => "installation_kind_mismatch",
            Self::RestoreRecord(_) => "restore_record_failed",
            _ => "internal",
        }
    }
}

impl From<InstallationError> for EngineError {
    fn from(error: InstallationError) -> Self {
        match error {
            InstallationError::KindMismatch { expected, found } => {
                Self::InstallationKindMismatch { expected, found }
            }
            InstallationError::Store(error) => Self::Store(error),
        }
    }
}
