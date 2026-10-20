//! SQLite system of record (NFR-01, NFR-02, NFR-06, NFR-10, REQ-OPS-001,
//! REQ-INT-008, ADR 0012).
//!
//! - [`Database`]: opens the data directory under a single-instance lock, sets
//!   the pragmas (WAL, `synchronous=FULL`, foreign keys, busy timeout),
//!   applies module migrations with checksums, and runs every write on one
//!   writer thread in a `BEGIN IMMEDIATE` transaction. Reads run on read-only
//!   connections.
//! - [`CommandContext`]: what a command sees inside its transaction:
//!   `tx()`, `principal()`, `now()`, `correlation_id()`, `require(capability)`,
//!   `audit(event)` and `enqueue(item)`. The change, its audit events and its
//!   outbox items commit or roll back together.
//! - [`outbox`]: durable follow-up work with leases, retries and dead letters.
//! - [`ArtifactStore`]: content-addressed files, synced before use, and
//!   listed with `digests` for backups and exports.
//! - [`lint_schema`]: the STRICT, no-REAL/ANY and table-prefix rules, for
//!   every module's tests.
//! - [`applied_migrations`] reads the migration ledger and [`check_applied`]
//!   checks a ledger from another file against this build, for backup,
//!   restore and exit manifests.
//! - [`Database::snapshot`]: `VACUUM INTO` snapshots with an integrity check;
//!   `Database::open` keeps one in [`PRE_MIGRATION_DIR`] before migrating an
//!   existing database (NFR-14).
//!
//! Invariants: only the writer thread writes; a command's writes commit only
//! if it returns `Ok`; callback and migration SQL cannot control the store's
//! transaction; an applied migration is never re-run or silently changed;
//! outbox rows, not memory, carry pending work across crashes.
//!
//! Domain crates use `?` on every `CommandContext` method by wrapping
//! [`StoreError`] in their own error enum:
//!
//! ```
//! use lis_audit::{AuditEvent, Outcome, Stream};
//! use lis_core::Capability;
//! use lis_store::{CommandContext, OutboxItem, StoreError};
//!
//! #[derive(Debug, thiserror::Error)]
//! pub enum SpecimenError {
//!     #[error("specimen not found")]
//!     NotFound,
//!     #[error(transparent)]
//!     Store(#[from] StoreError),
//! }
//!
//! impl From<rusqlite::Error> for SpecimenError {
//!     fn from(error: rusqlite::Error) -> Self {
//!         Self::Store(error.into())
//!     }
//! }
//!
//! /// Records receipt of a specimen (example only).
//! pub fn receive_specimen(ctx: &mut CommandContext<'_>, label: &str) -> Result<i64, SpecimenError> {
//!     ctx.require(Capability::Accession)?;
//!     let revision: i64 = ctx.tx().query_row("SELECT 1", [], |row| row.get(0))?;
//!     ctx.audit(
//!         AuditEvent::new(Stream::Clinical, "specimen.receive", Outcome::Succeeded)
//!             .record("specimen", label)
//!             .new_revision(revision),
//!     )?;
//!     ctx.enqueue(OutboxItem {
//!         topic: "label.print",
//!         dedup_key: label.to_owned(),
//!         payload: serde_json::json!({ "specimen": label }),
//!     })?;
//!     Ok(revision)
//! }
//! ```
//!
//! Network-path refusal of the data directory is an M4 health check and is
//! not done here.
//!
//! See `docs/architecture/overview.md#transaction-and-failure-design`.

mod artifacts;
mod context;
mod database;
mod error;
mod lint;
mod migrate;
pub mod outbox;
mod snapshot;
mod threads;
mod transaction_guard;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

use lis_core::{Migration, ModuleSchema};

pub use artifacts::{
    ArtifactDigest, ArtifactListing, ArtifactStore, MAX_ARTIFACT_BYTES, StoredArtifact, sync_dir,
};
pub use context::CommandContext;
pub use database::{ARTIFACTS_DIR, DATABASE_FILE, Database, WriteCommand};
pub use error::{MigrationProblem, StoreError};
pub use lint::{SchemaProblem, SchemaViolation, lint_schema};
pub use migrate::{AppliedMigration, applied_migrations, check_applied};
pub use outbox::{OutboxItem, OutboxItemId};
pub use snapshot::{PRE_MIGRATION_DIR, Snapshot};

/// This module's migrations, in version order.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "store_outbox",
    sql: include_str!("../migrations/0001_store_outbox.sql"),
}];

/// This module's schema descriptor. `Database::open` always applies it, after `lis_audit::SCHEMA`.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "store",
    table_prefix: "store_",
    migrations: MIGRATIONS,
};
