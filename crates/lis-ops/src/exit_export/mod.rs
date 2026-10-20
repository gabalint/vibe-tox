//! The exit export (REQ-OPS-010, REQ-OPS-003 export part, NFR-12; M4 design brief D16,
//! §5): a plaintext folder package that lets the laboratory leave with all of its data
//! and read it without this application. The package format is specified in
//! `docs/architecture/backup-and-export-formats.md` (exit export), whose section is also
//! the package's `README.txt`.
//!
//! The engine calls [`request_exit_export`] in the command (`export` capability, fresh
//! authentication, a reason), then [`run_exit_export`] inside `spawn_blocking` with the
//! folder the dialog returned; the path is never stored.
//!
//! Invariants: the audit streams are sealed first; the database copy is an
//! integrity-checked snapshot; every artifact is digest-verified while it is copied, and
//! one that cannot be read is listed in the manifest, never silently left out; the
//! manifest is written last and every digest it lists is read back; on failure the
//! package folder the application created is removed; the package holds PHI, so its
//! name, the README and the logs never do, and audit details hold IDs, codes, counts and
//! digests only.

mod contents;
mod dictionary;
mod package;
mod repo;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, CorrelationId, Id, ModuleSchema, Principal, required_text};
use lis_store::{CommandContext, Database};

pub use package::{EXIT_FORMAT, ExitManifest, ManifestFile};
pub use repo::{ExitExportRecord, list_exit_exports};

use crate::backup::{BackupError, JOBS_COMPONENT};
use package::{Package, Produced};

/// Audit action of an exit-export request.
pub const REQUEST_EXIT_EXPORT: &str = "ops.request_exit_export";
/// Audit action of an export's completion: succeeded, failed or interrupted.
pub const EXIT_EXPORT: &str = "ops.exit_export";
const REASON_MAX_CHARS: usize = 1000;

/// Marker type of exit export IDs.
pub enum ExitExportRun {}
/// Identifies an exit export.
pub type ExitExportId = Id<ExitExportRun>;

lis_core::text_enum! {
    /// Where an exit export stands (M4 design brief §2.3).
    ExportState {
        /// Recorded; the engine runs it at once.
        Requested = "requested",
        /// Being written.
        Running = "running",
        /// Written and read back.
        Succeeded = "succeeded",
        /// See the failure code; the package folder was removed.
        Failed = "failed",
        /// Stopped by shutdown or a crash.
        Interrupted = "interrupted",
    }
}

lis_core::text_enum! {
    /// Why an exit export failed.
    ExportFailure {
        /// The package folder could not be created in the chosen folder.
        FolderUnusable = "folder_unusable",
        /// The snapshot could not be written or read.
        SnapshotFailed = "snapshot_failed",
        /// The snapshot failed its integrity check: database damage.
        SnapshotNotIntact = "snapshot_not_intact",
        /// The artifact folder could not be listed.
        ArtifactListingFailed = "artifact_listing_failed",
        /// Writing or syncing a file failed, for example a full or removed drive.
        WriteFailed = "write_failed",
        /// A file read back differs from what the manifest lists.
        ReadbackMismatch = "readback_mismatch",
    }
}

/// Records an exit export into `folder` (`ops.request_exit_export`, with the reason).
/// Refused for a folder that does not exist or is inside `data_dir`, and while another
/// export waits or runs. Only the folder's last name is stored, for display.
pub fn request_exit_export(
    ctx: &mut CommandContext<'_>,
    data_dir: &Path,
    folder: &Path,
    reason: &str,
) -> Result<ExitExportId, BackupError> {
    ctx.require(Capability::Export)?;
    let reason = required_text("reason", reason, REASON_MAX_CHARS).map_err(BackupError::Invalid)?;
    let folder = fs::canonicalize(folder).map_err(|_| BackupError::ExportFolderUnusable)?;
    if !folder.is_dir() || folder.starts_with(fs::canonicalize(data_dir)?) {
        return Err(BackupError::ExportFolderUnusable);
    }
    if repo::any_unfinished(ctx.tx())? {
        return Err(BackupError::ExitExportRunning);
    }
    let label = folder
        .file_name()
        .map_or_else(|| folder.to_string_lossy(), |name| name.to_string_lossy())
        .into_owned();
    let export_id = ExitExportId::generate();
    let requested_by = ctx.principal().actor().id_text();
    repo::insert_request(ctx.tx(), export_id, &requested_by, &label, ctx.now())?;
    ctx.audit(
        AuditEvent::new(Stream::Security, REQUEST_EXIT_EXPORT, Outcome::Succeeded)
            .record("exit_export", export_id)
            .new_revision(1)
            .reason(reason),
    )?;
    Ok(export_id)
}

/// What the engine hands an exit export.
pub struct ExitExportJob<'a> {
    /// The live database.
    pub db: &'a Database,
    /// The data folder; the chosen folder must be outside it.
    pub data_dir: &'a Path,
    /// The requested export.
    pub export_id: ExitExportId,
    /// The folder the dialog returned; the package folder is created inside it.
    pub folder: &'a Path,
    /// This build's schema modules in crate-map order (the engine's `MODULES`), for the
    /// migration sources and the owning module of each table.
    pub modules: &'a [ModuleSchema],
    /// Every column holding artifact digests, as `(table, column)`.
    pub artifact_columns: &'a [(&'a str, &'a str)],
    /// Raised by shutdown; the export then ends `interrupted` and its folder is removed.
    pub cancel: &'a AtomicBool,
}

/// How an export ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitExportReport {
    /// The export.
    pub export_id: ExitExportId,
    /// `succeeded`, `failed` or `interrupted`.
    pub state: ExportState,
    /// Why it failed.
    pub failure: Option<ExportFailure>,
    /// The package folder's name, when it succeeded.
    pub package_name: Option<String>,
    /// SHA-256 of `manifest.json`, when it succeeded.
    pub manifest_sha256: Option<String>,
    /// SHA-256 of the packaged verifier, for comparison with a trusted copy.
    pub verifier_sha256: Option<String>,
    /// Artifacts the package lacks; listed in its manifest.
    pub artifact_problems: usize,
}

/// Writes the requested export's package and records the outcome (`ops.exit_export`,
/// system principal `ops-jobs`).
pub fn run_exit_export(job: &ExitExportJob<'_>) -> Result<ExitExportReport, BackupError> {
    let correlation = CorrelationId::generate();
    let export_id = job.export_id;
    let started = job.db.write_blocking(jobs(), correlation, move |ctx| {
        package::start(ctx, export_id)
    })?;
    let produced = Package::new(job, &started).write();
    let report = job.db.write_blocking(jobs(), correlation, move |ctx| {
        record(ctx, &started, produced)
    })?;
    tracing::info!(
        export_id = %report.export_id,
        state = report.state.as_str(),
        failure = report.failure.map(ExportFailure::as_str),
        artifact_problems = report.artifact_problems,
        "exit export finished"
    );
    Ok(report)
}

fn jobs() -> Principal {
    Principal::system(JOBS_COMPONENT, [])
}

fn record(
    ctx: &mut CommandContext<'_>,
    started: &package::Started,
    produced: Produced,
) -> Result<ExitExportReport, BackupError> {
    let mut report = ExitExportReport {
        export_id: started.export_id,
        state: ExportState::Failed,
        failure: None,
        package_name: None,
        manifest_sha256: None,
        verifier_sha256: None,
        artifact_problems: 0,
    };
    let written = match produced {
        Produced::Failed(failure) => {
            report.failure = Some(failure);
            None
        }
        Produced::Interrupted => {
            report.state = ExportState::Interrupted;
            None
        }
        Produced::Written(written) => {
            report.state = ExportState::Succeeded;
            report.package_name = Some(written.package_name.clone());
            report.manifest_sha256 = Some(written.manifest_sha256.clone());
            report.verifier_sha256 = Some(written.verifier_sha256.clone());
            report.artifact_problems = written.artifact_problems;
            Some(written)
        }
    };
    let revision = repo::finish(ctx.tx(), &report, written.as_deref(), ctx.now())?;
    let mut event = AuditEvent::new(Stream::Security, EXIT_EXPORT, outcome(report.state))
        .record("exit_export", report.export_id)
        .old_revision(revision - 1)
        .new_revision(revision)
        .detail("state", AuditDetail::code(report.state.as_str()));
    if let Some(failure) = report.failure {
        event = event.detail("failure_code", AuditDetail::code(failure.as_str()));
    }
    if let Some(written) = &written {
        event = event
            .detail(
                "manifest_sha256",
                AuditDetail::sha256(&written.manifest_sha256)?,
            )
            .detail(
                "verifier_sha256",
                AuditDetail::sha256(&written.verifier_sha256)?,
            )
            .detail("artifact_count", AuditDetail::count(written.artifact_count))
            .detail(
                "artifact_problems",
                AuditDetail::count(written.artifact_problems),
            )
            .detail(
                "database_bytes",
                AuditDetail::unsigned(written.database_bytes),
            );
    }
    ctx.audit(event)?;
    Ok(report)
}

fn outcome(state: ExportState) -> Outcome {
    if state == ExportState::Succeeded {
        Outcome::Succeeded
    } else {
        Outcome::Failed
    }
}

/// The audit event of an export a stop or crash left `requested` or `running`.
pub(crate) fn interrupted_event(export_id: ExitExportId, revision: i64) -> AuditEvent {
    AuditEvent::new(Stream::Security, EXIT_EXPORT, Outcome::Failed)
        .record("exit_export", export_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .detail(
            "state",
            AuditDetail::code(ExportState::Interrupted.as_str()),
        )
}

pub(crate) use repo::{interrupt, unfinished};
