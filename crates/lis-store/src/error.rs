//! The store's single error type.

use lis_audit::AuditError;
use lis_core::{Denied, ErrorCode};

/// Why a migration check refused to open the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationProblem {
    /// An applied migration's SQL differs from this build's.
    ChecksumChanged,
    /// A version is absent: a gap in the applied history, or an applied
    /// migration (or whole module) this build does not know.
    Missing,
    /// This build's migrations are not numbered 1, 2, 3… in order, or a module repeats.
    OutOfOrder,
    /// A migration left a row whose foreign key matches no parent row; it was
    /// rolled back.
    ForeignKeyViolation,
}

/// Failure of a store operation. `Display` is PHI-free and meant for logs.
///
/// Domain error enums wrap it with `#[error(transparent)] Store(#[from]
/// StoreError)`, so `?` works on every `CommandContext` method.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The principal lacks a required capability.
    #[error(transparent)]
    Denied(#[from] Denied),
    /// Another process holds the data directory's lock.
    #[error("another instance has this database open")]
    AlreadyOpen,
    /// The file system refused WAL journaling.
    #[error("the data directory does not support WAL journaling")]
    WalUnavailable,
    /// A migration check failed.
    #[error("migration {module} {version}: {problem:?}")]
    Migration {
        /// Module name.
        module: String,
        /// Migration version.
        version: u32,
        /// What is wrong.
        problem: MigrationProblem,
    },
    /// The copy of the database taken before migrating failed its integrity
    /// check; nothing was migrated (NFR-14).
    #[error("the pre-migration snapshot is not intact; nothing was migrated")]
    PreMigrationSnapshotNotIntact,
    /// The copy of the database taken before migrating could not be written;
    /// nothing was migrated (NFR-14).
    #[error("the pre-migration snapshot failed: {0}")]
    PreMigrationSnapshotFailed(Box<StoreError>),
    /// Same dedup key, different payload.
    #[error("outbox item already exists with a different payload")]
    OutboxConflict,
    /// The item is not leased under this attempt.
    #[error("outbox item is not leased by this attempt")]
    OutboxNotLeased,
    /// Time arithmetic left the supported range.
    #[error("timestamp out of range")]
    TimeOutOfRange,
    /// No artifact has this digest.
    #[error("artifact not found")]
    ArtifactNotFound,
    /// The artifact exceeds the size limit.
    #[error("artifact exceeds the size limit")]
    ArtifactTooLarge,
    /// A stored artifact's bytes do not match its digest.
    #[error("artifact content does not match its digest")]
    ArtifactCorrupt,
    /// Not a lowercase hex SHA-256 digest.
    #[error("not a valid artifact digest")]
    InvalidDigest,
    /// The writer thread stopped (after a panic); no more writes are possible.
    #[error("the database writer has stopped")]
    WriterStopped,
    /// The reader running the query panicked before answering, or every reader has stopped.
    #[error("the database readers have stopped")]
    ReadersStopped,
    /// A write was started from inside a command, which would wait on itself.
    #[error("a write cannot start inside another write")]
    NestedWrite,
    /// Callback SQL attempted to begin, end or split the store-owned transaction.
    #[error("transaction control is reserved for the store")]
    TransactionEnded,
    /// Appending an audit event failed.
    #[error(transparent)]
    Audit(#[from] AuditError),
    /// SQLite failed.
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    /// The file system failed.
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
}

impl StoreError {
    /// The `CommandError` code the engine reports for this error.
    ///
    /// For `Internal`, the engine shows a generic message and logs this
    /// error's `Display` with the correlation ID.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Denied(_) => ErrorCode::Forbidden,
            Self::OutboxConflict => ErrorCode::Conflict,
            Self::ArtifactNotFound => ErrorCode::NotFound,
            Self::ArtifactTooLarge | Self::InvalidDigest => ErrorCode::Validation,
            Self::AlreadyOpen | Self::WriterStopped | Self::ReadersStopped => {
                ErrorCode::Unavailable
            }
            Self::WalUnavailable
            | Self::Migration { .. }
            | Self::PreMigrationSnapshotNotIntact
            | Self::PreMigrationSnapshotFailed(_)
            | Self::NestedWrite
            | Self::TransactionEnded
            | Self::OutboxNotLeased
            | Self::TimeOutOfRange
            | Self::ArtifactCorrupt
            | Self::Audit(_)
            | Self::Sql(_)
            | Self::Io(_) => ErrorCode::Internal,
        }
    }
}
