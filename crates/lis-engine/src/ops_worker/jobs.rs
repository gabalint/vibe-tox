//! One job-runner pass (M4 design brief §3.1, §3.4, §3.6, §4, §5): jobs run one at a time,
//! on a blocking thread, as the `ops-jobs` system principal:
//!
//! 1. backups: the oldest waiting request first, then scheduled copies to due targets,
//!    until none is left (`lis_ops::backup::run` seals before each snapshot);
//! 2. seals and anchor export at least every [`SEAL_INTERVAL_MINUTES`] and after each
//!    backup;
//! 3. requested restore tests;
//! 4. requested exit exports, with the folder their dialog returned;
//! 5. chain and anchor verification once a day, and at the first pass after a start.
//!
//! The first pass after a start therefore catches up overdue backups, seals and anchors.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use lis_core::{Principal, UtcTimestamp};
use lis_ops::backup::{JOBS_COMPONENT, RunFailure, RunReport};
use lis_ops::exit_export::{ExitExportId, ExitExportJob};
use lis_ops::format::BackupKey;
use lis_ops::policy::SEAL_INTERVAL_MINUTES;
use lis_ops::restore::RestoreTestJob;

use super::{Operations, OpsError, health, locked};
use crate::engine::{MODULES, artifact_columns};

/// Chains and anchors are verified this often.
const VERIFY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// A job the close warning names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobKind {
    Backup,
    AnchorExport,
    RestoreTest,
    ExitExport,
    Verification,
}

impl JobKind {
    /// PHI-free text for the close warning.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Backup => "A backup is being written.",
            Self::AnchorExport => "Audit anchors are being written.",
            Self::RestoreTest => "A restore test is running.",
            Self::ExitExport => "An exit export is being written.",
            Self::Verification => "The audit trail is being verified.",
        }
    }
}

/// When the periodic jobs last ran, by the engine clock.
#[derive(Debug, Default)]
pub(crate) struct Schedule {
    last_seal: Option<UtcTimestamp>,
    last_verification: Option<UtcTimestamp>,
}

/// One pass; a failure is logged and the next pass retries.
pub(crate) async fn run_pass(ops: &Arc<Operations>) {
    if let Err(error) = pass(ops).await {
        tracing::warn!(%error, "job runner pass failed; retrying at the next pass");
    }
}

async fn pass(ops: &Arc<Operations>) -> Result<(), OpsError> {
    let settings = ops.db.read(lis_ops::settings::current).await?;
    let seal_interval = Duration::from_secs(u64::from(SEAL_INTERVAL_MINUTES) * 60);
    let mut copied = false;
    while !cancelled(ops) {
        let key = installation_key(ops);
        let report = blocking(ops, JobKind::Backup, move |ops| {
            let key = key.as_ref().map(BackupKey::derive);
            lis_ops::backup::run(
                &ops.db,
                &ops.data_dir,
                key.as_ref(),
                &settings,
                ops.clock.as_ref(),
                &ops.cancel,
            )
        })
        .await??;
        let Some(report) = report else {
            break;
        };
        copied |= report.copies_verified > 0;
        after_backup(ops, &report).await;
    }
    let now = ops.clock.now();
    if !cancelled(ops) && (copied || due(locked(&ops.schedule).last_seal, now, seal_interval)) {
        blocking(ops, JobKind::AnchorExport, |ops| {
            lis_ops::anchors::export_pending(&ops.db, Principal::system(JOBS_COMPONENT, []))
        })
        .await??;
        locked(&ops.schedule).last_seal = Some(now);
    }
    while !cancelled(ops) {
        let key = installation_key(ops);
        let report = blocking(ops, JobKind::RestoreTest, move |ops| {
            let key = key.as_ref().map(BackupKey::derive);
            let columns = artifact_columns();
            lis_ops::restore::run_restore_test(&RestoreTestJob {
                db: &ops.db,
                data_dir: &ops.data_dir,
                key: key.as_ref(),
                modules: MODULES,
                artifact_columns: &columns,
                clock: Arc::clone(&ops.clock),
                cancel: &ops.cancel,
            })
        })
        .await??;
        if report.is_none() {
            break;
        }
    }
    while !cancelled(ops) {
        let Some((export_id, folder)) = locked(&ops.exit_exports).pop_front() else {
            break;
        };
        exit_export(ops, export_id, folder).await?;
    }
    if !cancelled(ops)
        && due(
            locked(&ops.schedule).last_verification,
            now,
            VERIFY_INTERVAL,
        )
    {
        verify(ops).await?;
        locked(&ops.schedule).last_verification = Some(now);
    }
    Ok(())
}

/// Raises the `artifact_store`, `backup_failures` and `database_integrity` findings at
/// once instead of at the next health step.
async fn after_backup(ops: &Arc<Operations>, report: &RunReport) {
    if report.failure == Some(RunFailure::SnapshotNotIntact) {
        tracing::error!(
            run_id = %report.run_id,
            "the database snapshot failed its integrity check: possible database damage"
        );
    }
    if (report.artifact_problems > 0 || report.copies_failed > 0 || report.failure.is_some())
        && let Err(error) = health::evaluate_and_record(ops, false).await
    {
        tracing::warn!(%error, "health evaluation after a backup failed");
    }
}

async fn exit_export(
    ops: &Arc<Operations>,
    export_id: ExitExportId,
    folder: PathBuf,
) -> Result<(), OpsError> {
    blocking(ops, JobKind::ExitExport, move |ops| {
        let columns = artifact_columns();
        lis_ops::exit_export::run_exit_export(&ExitExportJob {
            db: &ops.db,
            data_dir: &ops.data_dir,
            export_id,
            folder: &folder,
            modules: MODULES,
            artifact_columns: &columns,
            cancel: &ops.cancel,
        })
    })
    .await??;
    Ok(())
}

/// Verifies every chain and the anchors on every available target (NFR-18); the count of
/// problems feeds the `audit_chain` check.
async fn verify(ops: &Arc<Operations>) -> Result<(), OpsError> {
    let problems = blocking(ops, JobKind::Verification, |ops| {
        ops.db.read_blocking(|conn| -> Result<usize, OpsError> {
            let folders: Vec<PathBuf> = lis_ops::targets::active(conn)?
                .iter()
                .filter(|target| lis_ops::targets::is_available(target))
                .map(lis_ops::targets::root)
                .collect();
            let read = lis_ops::anchors::read_anchors(&folders)?;
            let restores = lis_ops::restore::restore_records(conn)?;
            Ok(lis_ops::anchors::verify_anchors(conn, &read, &restores)?.problems())
        })
    })
    .await??;
    let problems = u32::try_from(problems).unwrap_or(u32::MAX);
    if problems > 0 {
        tracing::error!(
            problems,
            "audit chain or anchor verification found problems"
        );
    }
    locked(&ops.health).chain_findings = Some(problems);
    Ok(())
}

fn due(last: Option<UtcTimestamp>, now: UtcTimestamp, interval: Duration) -> bool {
    last.is_none_or(|last| now.saturating_duration_since(last) >= interval)
}

fn cancelled(ops: &Operations) -> bool {
    ops.cancel.load(Ordering::SeqCst)
}

fn installation_key(ops: &Operations) -> Option<lis_auth::DataKey> {
    locked(&ops.key).usable().ok()
}

/// Runs `job` on a blocking thread, named in the close warning while it runs.
async fn blocking<T, F>(ops: &Arc<Operations>, kind: JobKind, job: F) -> Result<T, OpsError>
where
    F: FnOnce(&Operations) -> T + Send + 'static,
    T: Send + 'static,
{
    *locked(&ops.running) = Some(kind);
    let shared = Arc::clone(ops);
    let result = tokio::task::spawn_blocking(move || job(&shared)).await;
    *locked(&ops.running) = None;
    result.map_err(|_| OpsError::Task)
}
