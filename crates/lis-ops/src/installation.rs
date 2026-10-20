//! The installation's identity: one row, written at the first start and never changed. Its
//! kind, laboratory or demo, is fixed then too (proposed REQ-OPS-016): the app opens a
//! database only as the kind it was created as.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{ErrorCode, InstallationId};
use lis_store::{CommandContext, StoreError};
use rusqlite::{Connection, OptionalExtension, params};

/// Audit action of the first start, when the installation row is written.
pub const INSTALLATION_CREATE: &str = "ops.installation_create";

lis_core::text_enum! {
    /// What an installation is for, fixed when its database is created (proposed
    /// REQ-OPS-016; starter data brief, decision 1a).
    #[ts(export)]
    InstallationKind {
        /// A laboratory's own installation: its system of record.
        Laboratory = "laboratory",
        /// The demo laboratory: fictitious data in its own data folder, never a laboratory's.
        Demo = "demo",
    }
}

/// Why the installation could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum InstallationError {
    /// The database was created as the other kind: a demo database never opens as a
    /// laboratory, nor the reverse.
    #[error("this data folder holds a {found} installation, not a {expected} one")]
    KindMismatch {
        /// The kind the app was asked to open.
        expected: &'static str,
        /// The kind the database was created as.
        found: &'static str,
    },
    /// Database or audit failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl InstallationError {
    /// Stable classification.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::KindMismatch { .. } => ErrorCode::Conflict,
            Self::Store(error) => error.code(),
        }
    }
}

/// This installation's ID, or `None` before the first start has recorded it.
pub fn installation_id(conn: &Connection) -> Result<Option<InstallationId>, StoreError> {
    Ok(conn
        .prepare_cached("SELECT installation_id FROM ops_installation WHERE singleton = 1")?
        .query_row([], |row| row.get(0))
        .optional()?)
}

/// This installation's kind, or `None` before the first start has recorded it.
pub fn installation_kind(conn: &Connection) -> Result<Option<InstallationKind>, StoreError> {
    Ok(conn
        .prepare_cached("SELECT kind FROM ops_installation WHERE singleton = 1")?
        .query_row([], |row| row.get(0))
        .optional()?)
}

/// Returns the installation's ID, creating and auditing a laboratory installation on the
/// first start. The engine opens with [`open_installation`], which also checks the kind;
/// this form serves fixtures of laboratory databases.
pub fn ensure_installation(ctx: &mut CommandContext<'_>) -> Result<InstallationId, StoreError> {
    if let Some(id) = installation_id(ctx.tx())? {
        return Ok(id);
    }
    create(ctx, InstallationKind::Laboratory)
}

/// Returns the installation's ID, creating it as `kind` on the first start. A database
/// created as the other kind is refused, so a demo database is never used as a laboratory's
/// and the reverse (proposed REQ-OPS-016).
pub fn open_installation(
    ctx: &mut CommandContext<'_>,
    kind: InstallationKind,
) -> Result<InstallationId, InstallationError> {
    let (Some(id), Some(found)) = (installation_id(ctx.tx())?, installation_kind(ctx.tx())?) else {
        return Ok(create(ctx, kind)?);
    };
    if found != kind {
        return Err(InstallationError::KindMismatch {
            expected: kind.as_str(),
            found: found.as_str(),
        });
    }
    Ok(id)
}

fn create(
    ctx: &mut CommandContext<'_>,
    kind: InstallationKind,
) -> Result<InstallationId, StoreError> {
    let id = InstallationId::generate();
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_installation (singleton, installation_id, created_at, kind) \
             VALUES (1, ?1, ?2, ?3)",
        )?
        .execute(params![id, ctx.now(), kind])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, INSTALLATION_CREATE, Outcome::Succeeded)
            .record("installation", id)
            .detail("kind", AuditDetail::code(kind.as_str())),
    )?;
    Ok(id)
}
