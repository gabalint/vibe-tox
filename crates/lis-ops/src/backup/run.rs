//! One backup run, for the engine's serial job runner (M4 design brief §3.1). Blocking:
//! the engine calls it inside `spawn_blocking`. Long file I/O runs between two short
//! write transactions, never inside one.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use lis_audit::ChainLink;
use lis_core::{Clock, CorrelationId, InstallationId, Principal, UtcTimestamp};
use lis_store::{ARTIFACTS_DIR, CommandContext, Database};

use super::copy::{CopyError, SetSource, copy_set, discard_set};
use super::manifest::{
    ArtifactProblem, DatabaseEntry, MANIFEST_FORMAT, Manifest, head_entry, read_snapshot, set_id,
    unexpected_entries,
};
use super::record::finish;
use super::repo;
use super::retention::prune;
use super::staging::{SNAPSHOT_SUFFIX, clear_staging, enough_space, prepare_staging, staging_dir};
use super::{BackupError, CopyFailure, RunFailure, RunId, RunKind, RunState, plan};
use crate::format::BackupKey;
use crate::installation_id;
use crate::policy::OperationsSettings;
use crate::targets::{self, Target, TargetId};

/// The system principal of backup completions (`ops.backup`).
pub const JOBS_COMPONENT: &str = "ops-jobs";

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// The run.
    pub run_id: RunId,
    /// Why it started.
    pub kind: RunKind,
    /// `succeeded`, `failed` or `interrupted`.
    pub state: RunState,
    /// Why it failed.
    pub failure: Option<RunFailure>,
    /// Targets that received a verified set.
    pub copies_verified: usize,
    /// Targets whose copy failed.
    pub copies_failed: usize,
    /// Artifacts that could not be copied, plus unexpected entries of the artifact folder;
    /// above 0 the engine raises the `artifact_store` alert.
    pub artifact_problems: usize,
    /// Sets retention deleted.
    pub sets_pruned: usize,
}

/// Runs the next backup, if any: the oldest waiting request (to every available target
/// that takes backups), otherwise a scheduled run to the due targets. Returns `None`
/// when there is nothing to do. `key` is `None` when the installation key is not loaded:
/// the run then fails with `key_unavailable`. Raising `cancel` stops the run between
/// files and chunks; it ends `interrupted` and leaves no set.
pub fn run(
    db: &Database,
    data_dir: &Path,
    key: Option<&BackupKey>,
    settings: &OperationsSettings,
    clock: &dyn Clock,
    cancel: &AtomicBool,
) -> Result<Option<RunReport>, BackupError> {
    let available = plan::available_backup_targets(db.read_blocking(targets::active)?);
    let plan_settings = *settings;
    let correlation = CorrelationId::generate();
    let Some(started) = db.write_blocking(jobs(), correlation, move |ctx| {
        start(ctx, &plan_settings, available)
    })?
    else {
        return Ok(None);
    };
    let job = Job {
        db,
        data_dir,
        settings,
        clock,
        cancel,
    };
    let outcome = match key {
        Some(key) => produce(&job, key, &started),
        None => Produced::Failed(RunFailure::KeyUnavailable),
    };
    let report = db.write_blocking(jobs(), correlation, move |ctx| {
        finish(ctx, &started, outcome)
    })?;
    tracing::info!(
        run_id = %report.run_id,
        kind = report.kind.as_str(),
        state = report.state.as_str(),
        failure = report.failure.map(RunFailure::as_str),
        copies_verified = report.copies_verified,
        copies_failed = report.copies_failed,
        artifact_problems = report.artifact_problems,
        "backup run finished"
    );
    Ok(Some(report))
}

fn jobs() -> Principal {
    Principal::system(JOBS_COMPONENT, [])
}

struct Job<'a> {
    db: &'a Database,
    data_dir: &'a Path,
    settings: &'a OperationsSettings,
    clock: &'a dyn Clock,
    cancel: &'a AtomicBool,
}

/// A run moved to `running`, with the targets it copies to.
pub(super) struct Started {
    pub(super) run_id: RunId,
    pub(super) kind: RunKind,
    pub(super) requested_by: Option<String>,
    pub(super) started_at: UtcTimestamp,
    pub(super) revision: i64,
    pub(super) installation: InstallationId,
    pub(super) targets: Vec<Target>,
}

/// What the file work produced.
pub(super) enum Produced {
    Failed(RunFailure),
    Interrupted,
    Copied(Box<Copies>),
}

/// What a run with an intact snapshot copied and pruned.
pub(super) struct Copies {
    pub(super) snapshot_sha256: String,
    pub(super) snapshot_bytes: u64,
    pub(super) heads: Vec<ChainLink>,
    pub(super) set_id: String,
    pub(super) artifact_count: usize,
    pub(super) artifact_problems: usize,
    pub(super) copies: Vec<TargetCopy>,
    pub(super) pruned: Vec<(TargetId, Vec<String>)>,
    pub(super) prune_failures: usize,
}

/// One target's copy.
pub(super) struct TargetCopy {
    pub(super) target_id: TargetId,
    pub(super) written_bytes: u64,
    /// When the set was verified, or why the copy failed.
    pub(super) result: Result<UtcTimestamp, CopyFailure>,
}

/// Picks the work, inserts or starts the run and seals the audit streams (§3.1 steps 1–2).
fn start(
    ctx: &mut CommandContext<'_>,
    settings: &OperationsSettings,
    available: Vec<Target>,
) -> Result<Option<Started>, BackupError> {
    let installation = installation_id(ctx.tx())?.ok_or(BackupError::NoInstallation)?;
    let now = ctx.now();
    let (run_id, kind, requested_by, revision, targets) = match repo::oldest_request(ctx.tx())? {
        Some(request) => {
            let revision = repo::start_requested(ctx.tx(), request.id, now)?;
            (
                request.id,
                request.kind,
                Some(request.requested_by),
                revision,
                available,
            )
        }
        None => {
            let due = plan::due_targets(ctx.tx(), now, settings, available)?;
            if due.is_empty() {
                return Ok(None);
            }
            let run_id = RunId::generate();
            repo::insert_scheduled(ctx.tx(), run_id, now)?;
            (run_id, RunKind::Scheduled, None, 1, due)
        }
    };
    lis_audit::seal(ctx.tx(), now)?;
    Ok(Some(Started {
        run_id,
        kind,
        requested_by,
        started_at: now,
        revision,
        installation,
        targets,
    }))
}

/// Snapshots, copies to each target and prunes (§3.1 steps 3–7); the staging copy is
/// removed whatever happens.
fn produce(job: &Job<'_>, key: &BackupKey, run: &Started) -> Produced {
    if run.targets.is_empty() {
        return Produced::Failed(RunFailure::NoTargetAvailable);
    }
    let staging = staging_dir(job.data_dir);
    let snapshot = staging.join(format!("{}{SNAPSHOT_SUFFIX}", run.run_id));
    let outcome = match prepare_staging(&staging) {
        Ok(()) => snapshot_and_copy(job, key, run, &snapshot),
        Err(error) => {
            tracing::error!(run_id = %run.run_id, error_kind = ?error.kind(), "backup staging unavailable");
            Produced::Failed(RunFailure::SnapshotFailed)
        }
    };
    if let Err(error) = clear_staging(&staging) {
        tracing::warn!(run_id = %run.run_id, error_kind = ?error.kind(), "backup staging not cleared");
    }
    outcome
}

fn snapshot_and_copy(job: &Job<'_>, key: &BackupKey, run: &Started, path: &Path) -> Produced {
    match enough_space(job.data_dir) {
        Ok(true) => {}
        Ok(false) => return Produced::Failed(RunFailure::InsufficientSpace),
        Err(error) => {
            tracing::error!(run_id = %run.run_id, error_kind = ?error.kind(), "free space unknown");
            return Produced::Failed(RunFailure::InsufficientSpace);
        }
    }
    let snapshot = match job.db.snapshot(path) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::error!(run_id = %run.run_id, error = %error, "backup snapshot failed");
            return Produced::Failed(RunFailure::SnapshotFailed);
        }
    };
    if !snapshot.is_intact() {
        tracing::error!(run_id = %run.run_id, "backup snapshot failed its integrity check");
        return Produced::Failed(RunFailure::SnapshotNotIntact);
    }
    let Ok(facts) = read_snapshot(path) else {
        return Produced::Failed(RunFailure::SnapshotFailed);
    };
    let Ok(listing) = job.db.artifacts().digests() else {
        return Produced::Failed(RunFailure::ArtifactListingFailed);
    };
    let manifest = Manifest {
        format: MANIFEST_FORMAT.to_owned(),
        installation_id: run.installation,
        set_id: set_id(run.started_at, run.run_id),
        run_id: run.run_id,
        kind: run.kind,
        created_at: run.started_at,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        key_id: key.key_id().to_string(),
        schema: facts.schema,
        database: DatabaseEntry::intact(snapshot.digest.to_string(), snapshot.size),
        heads: facts.heads.iter().map(head_entry).collect(),
        seals: facts.seals,
        artifacts: Vec::new(),
        artifact_problems: unexpected_entries(&listing),
    };
    let store_dir = job.data_dir.join(ARTIFACTS_DIR);
    let source = SetSource {
        snapshot: path,
        store: job.db.artifacts(),
        store_dir: &store_dir,
        digests: &listing.digests,
        manifest: &manifest,
    };
    let Some(copies) = copy_to_targets(job, key, run, &source) else {
        return Produced::Interrupted;
    };
    let mut result = Copies {
        snapshot_sha256: snapshot.digest.to_string(),
        snapshot_bytes: snapshot.size,
        heads: facts.heads,
        set_id: manifest.set_id.clone(),
        artifact_count: listing.digests.len(),
        artifact_problems: copies.1.len() + listing.unexpected.len(),
        copies: copies.0,
        pruned: Vec::new(),
        prune_failures: 0,
    };
    prune_targets(job, key, run, &mut result);
    Produced::Copied(Box::new(result))
}

type CopyResults = (Vec<TargetCopy>, BTreeSet<ArtifactProblem>);

/// Copies the set to every target; `None` when cancelled, after removing every set this
/// run had already verified.
fn copy_to_targets(
    job: &Job<'_>,
    key: &BackupKey,
    run: &Started,
    source: &SetSource<'_>,
) -> Option<CopyResults> {
    let mut copies = Vec::new();
    let mut problems = BTreeSet::new();
    for target in &run.targets {
        let cancelled = job.cancel.load(Ordering::Relaxed);
        let copied = if cancelled {
            Err(CopyError::Cancelled)
        } else {
            copy_set(&targets::root(target), source, key, job.cancel)
        };
        match copied {
            Ok(copied) => {
                problems.extend(copied.problems);
                copies.push(TargetCopy {
                    target_id: target.id,
                    written_bytes: copied.written_bytes,
                    result: Ok(job.clock.now()),
                });
            }
            Err(CopyError::Failed {
                failure,
                written_bytes,
            }) => {
                tracing::warn!(run_id = %run.run_id, target_id = %target.id, failure = failure.as_str(), "backup copy failed");
                copies.push(TargetCopy {
                    target_id: target.id,
                    written_bytes,
                    result: Err(failure),
                });
            }
            Err(CopyError::Cancelled) => {
                for target in &run.targets {
                    discard_set(&targets::root(target), &source.manifest.set_id);
                }
                return None;
            }
        }
    }
    Some((copies, problems))
}

/// Applies retention on every target that received a verified set (§3.4).
fn prune_targets(job: &Job<'_>, key: &BackupKey, run: &Started, result: &mut Copies) {
    for target in &run.targets {
        let verified = result
            .copies
            .iter()
            .any(|copy| copy.target_id == target.id && copy.result.is_ok());
        if !verified {
            continue;
        }
        match prune(
            &targets::root(target),
            target.id,
            key,
            run.installation,
            job.settings,
        ) {
            Ok(pruned) => {
                result.prune_failures += pruned.failures;
                result.pruned.push((target.id, pruned.set_ids));
            }
            Err(error) => {
                tracing::warn!(run_id = %run.run_id, target_id = %target.id, error = %error, "backup retention skipped");
                result.prune_failures += 1;
            }
        }
    }
}
