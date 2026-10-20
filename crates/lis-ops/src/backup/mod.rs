//! Encrypted backup copies to folder targets, with retention (REQ-OPS-007, NFR-04,
//! NFR-11 retention part, NFR-20, NFR-30; M4 design brief §3.1, §3.3–§3.5).
//!
//! One [`run`] seals the audit streams, snapshots the database, and writes one `vtbk1`
//! set per due target: `sets/<setId>/database.vtbk` and `manifest.vtbk`, plus every
//! artifact the target lacks under `artifacts/<sha256>.vtbk`. A set is written under
//! `sets/<setId>.tmp`, read back and compared, and only then renamed into place and
//! recorded as a verified copy. Retention then prunes the target's older sets.
//!
//! Invariants:
//! - nothing is counted before it is read back: a copy row is `succeeded` only for a
//!   renamed, verified set, and an interrupted run leaves no set and no copy row;
//! - a snapshot that fails its integrity check is never copied;
//! - a missing or damaged artifact is counted in `artifact_problems` and in the manifest,
//!   never fatal; unexpected entries of the artifact folder are counted, never copied;
//! - a target is due when it is available, takes backups, its newest verified set's
//!   chain heads differ from the current ones (events written by the backup and anchor
//!   jobs themselves do not count), and the interval has passed since its last attempt;
//!   a newly plugged-in drive is therefore due at once;
//! - retention keeps the newest `keep_recent` sets, the newest per UTC day and per ISO
//!   week for the configured numbers of days and weeks that have sets, every manual and
//!   pre-update set, and always the newest set; it deletes only sets whose manifest names
//!   this installation, and a failed delete is a warning;
//! - no file I/O runs inside a write transaction, except the target marker in
//!   [`add_target`](crate::targets::add_target);
//! - audit details hold IDs, codes, counts and digests; logs hold IDs and codes; neither
//!   ever holds a path, label or key material.

mod copy;
pub(crate) mod durable;
mod error;
mod health_facts;
pub(crate) mod kit_text;
mod manifest;
mod plan;
mod query;
mod reconcile;
mod record;
mod repo;
mod retention;
mod run;
mod staging;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, Id};
use lis_store::CommandContext;

pub(crate) use copy::artifact_path;
pub use error::BackupError;
pub use health_facts::{health_facts, snapshot_growth_per_day};
pub use manifest::{
    ArtifactEntry, ArtifactProblem, DatabaseEntry, HeadEntry, MANIFEST_FORMAT, Manifest,
    SchemaEntry, SealEntry, SetListing, list_sets, read_manifest,
};
pub(crate) use manifest::{database_path, set_dir, set_names};
pub use plan::is_due;
pub use query::{
    CopyRecord, RunRecord, StatusRecord, TargetRecord, get_run, list_runs, list_runs_page,
    list_targets, status,
};
pub use reconcile::{PublishedSetReconciliation, reconcile_published_sets};
pub use run::{JOBS_COMPONENT, RunReport, run};
pub use staging::clean_staging;

use crate::{exit_export, restore, targets};

/// Audit action of a manual or pre-update backup request.
pub const REQUEST_BACKUP: &str = "ops.request_backup";
/// Audit action of a run's completion: succeeded, failed or interrupted.
pub const BACKUP: &str = "ops.backup";

/// Marker type of backup run IDs.
pub enum BackupRun {}
/// Identifies a backup run; the last 8 hex digits name its sets.
pub type RunId = Id<BackupRun>;

lis_core::text_enum! {
    /// Why a backup run started.
    RunKind {
        /// The interval passed and something changed.
        Scheduled = "scheduled",
        /// A user asked for it.
        Manual = "manual",
        /// A user asked for it before an update (REQ-OPS-006); never pruned.
        PreUpdate = "pre_update",
    }
}

lis_core::text_enum! {
    /// Where a backup run stands (M4 design brief §2.3).
    RunState {
        /// Waiting for the job runner.
        Requested = "requested",
        /// Being written.
        Running = "running",
        /// At least one copy was verified.
        Succeeded = "succeeded",
        /// No copy was verified; see the failure code.
        Failed = "failed",
        /// Stopped by shutdown or a crash; never verified.
        Interrupted = "interrupted",
    }
}

lis_core::text_enum! {
    /// Why a run failed.
    RunFailure {
        /// The installation key is not loaded (M4 design brief §3.9).
        KeyUnavailable = "key_unavailable",
        /// A requested run found no available target.
        NoTargetAvailable = "no_target_available",
        /// Free space on the data folder's disk is below 1.2 × the database + 1 GiB.
        InsufficientSpace = "insufficient_space",
        /// The snapshot could not be written or read.
        SnapshotFailed = "snapshot_failed",
        /// The snapshot failed its integrity check: database damage.
        SnapshotNotIntact = "snapshot_not_intact",
        /// The artifact folder could not be listed.
        ArtifactListingFailed = "artifact_listing_failed",
        /// Every copy failed.
        AllCopiesFailed = "all_copies_failed",
    }
}

lis_core::text_enum! {
    /// Why a copy to one target failed.
    CopyFailure {
        /// Writing, syncing or renaming failed, for example a full or removed drive.
        WriteFailed = "write_failed",
        /// The files read back differ from what was written.
        ReadbackMismatch = "readback_mismatch",
    }
}

/// Queues a manual or pre-update backup (`ops.request_backup`); the job runner runs it
/// ahead of scheduled work. Refused without an active target that takes backups, or
/// while another request waits.
pub fn request_backup(ctx: &mut CommandContext<'_>, kind: RunKind) -> Result<RunId, BackupError> {
    ctx.require(Capability::BackupRestore)?;
    if kind == RunKind::Scheduled {
        return Err(BackupError::Invalid(FieldError {
            field: "kind".to_owned(),
            code: "invalid".to_owned(),
            message: "Choose a manual or pre-update backup.".to_owned(),
        }));
    }
    let takes_backups = targets::active(ctx.tx())?
        .iter()
        .any(|target| target.purpose.takes_backups());
    if !takes_backups {
        return Err(BackupError::NoBackupTarget);
    }
    if repo::oldest_request(ctx.tx())?.is_some() {
        return Err(BackupError::RequestQueued);
    }
    let run_id = RunId::generate();
    let requested_by = ctx.principal().actor().id_text();
    repo::insert_request(ctx.tx(), run_id, kind, &requested_by, ctx.now())?;
    ctx.audit(
        AuditEvent::new(Stream::Security, REQUEST_BACKUP, Outcome::Succeeded)
            .record("backup_run", run_id)
            .new_revision(1)
            .detail("kind", AuditDetail::code(kind.as_str())),
    )?;
    Ok(run_id)
}

/// Marks backup runs, restore tests and exit exports left `requested` or `running` by a
/// stop or crash as `interrupted`, with one `ops.backup`, `ops.restore_test` or
/// `ops.exit_export` event each; the engine calls it at startup. Their sets were never
/// renamed into place, so nothing of them is listed; an exit package folder a crash left
/// behind has no `manifest.json`.
pub fn interrupt_abandoned(ctx: &mut CommandContext<'_>) -> Result<usize, BackupError> {
    let runs = repo::unfinished(ctx.tx())?;
    for (run_id, revision) in &runs {
        repo::interrupt(ctx.tx(), *run_id, ctx.now())?;
        ctx.audit(interrupted_event(*run_id, *revision))?;
    }
    let tests = restore::unfinished_tests(ctx.tx())?;
    for (test_id, revision) in &tests {
        restore::interrupt_test(ctx.tx(), *test_id, ctx.now())?;
        ctx.audit(restore::interrupted_test_event(*test_id, *revision))?;
    }
    let exports = exit_export::unfinished(ctx.tx())?;
    for (export_id, revision) in &exports {
        exit_export::interrupt(ctx.tx(), *export_id, ctx.now())?;
        ctx.audit(exit_export::interrupted_event(*export_id, *revision))?;
    }
    Ok(runs.len() + tests.len() + exports.len())
}

fn interrupted_event(run_id: RunId, revision: i64) -> AuditEvent {
    AuditEvent::new(Stream::Security, BACKUP, Outcome::Failed)
        .record("backup_run", run_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .detail("state", AuditDetail::code(RunState::Interrupted.as_str()))
}
