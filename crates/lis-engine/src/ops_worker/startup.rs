//! The operations steps of startup (M4 design brief §6.4 steps 2, 5 and 8): apply a
//! confirmed restore before the database opens, record an applied restore after it opens,
//! move unrecorded published backup sets aside, and record the app run with the jobs a
//! stop interrupted.

use std::path::Path;

use lis_auth::DataKey;
use lis_core::{Capability, CorrelationId, Principal, UtcTimestamp};
use lis_ops::backup::BackupError;
use lis_ops::format::BackupKey;
use lis_ops::restore::{ApplyOutcome, RestoreFloors};
use lis_ops::runs::{AppRunId, RunStart};
use lis_store::{Database, StoreError};

use crate::engine::{APP_VERSION, EngineError};

/// The system principal of startup and shutdown steps.
const LIFECYCLE: &str = "engine-lifecycle";

/// Runs `command` on the writer as the lifecycle principal.
pub(crate) async fn lifecycle_write<T, E, F>(db: &Database, command: F) -> Result<T, E>
where
    F: lis_store::WriteCommand<T, E>,
    T: Send + 'static,
    E: From<StoreError> + Send + 'static,
{
    db.write(
        Principal::system(LIFECYCLE, []),
        CorrelationId::generate(),
        command,
    )
    .await
}

/// Puts a confirmed restore in place before the database opens (§3.7). A refused restore
/// leaves the current database and is shown on the setup screen; an I/O failure stops
/// the start, and the next start resumes it.
pub(crate) async fn apply_restore(data_dir: &Path, now: UtcTimestamp) -> Result<(), EngineError> {
    let dir = data_dir.to_path_buf();
    let outcome = tokio::task::spawn_blocking(move || {
        lis_ops::backup::clean_staging(&dir)?;
        lis_ops::restore::apply_staged(&dir, now, |conn| {
            lis_auth::setup_status(conn).map_or(true, |status| !status.needs_administrator)
        })
    })
    .await
    .map_err(|_| EngineError::StartupTask)?
    .map_err(EngineError::RestoreApply)?;
    match outcome {
        ApplyOutcome::Nothing => {}
        ApplyOutcome::Applied(restore_id) => {
            tracing::info!(%restore_id, "restore in place; recording it after the open");
        }
        ApplyOutcome::Failed(failure) => {
            tracing::warn!(failure = failure.as_str(), "staged restore refused");
        }
    }
    Ok(())
}

/// Why recording an applied restore failed.
#[derive(Debug, thiserror::Error)]
enum RestoreRecordError {
    #[error(transparent)]
    Backup(#[from] BackupError),
    #[error(transparent)]
    Specimen(#[from] lis_specimens::SpecimenError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Records an applied restore as the `restore` principal: `ops.restore_applied`, the
/// reconciliation it opens, and the number floors raised by the specimens owner, in one
/// transaction (§3.7, §3.8).
pub(crate) async fn record_restore(db: &Database, data_dir: &Path) -> Result<(), EngineError> {
    let failed = |error: &dyn std::fmt::Display| EngineError::RestoreRecord(error.to_string());
    let Some(marker) = lis_ops::restore::applied_restore(data_dir).map_err(|e| failed(&e))? else {
        return Ok(());
    };
    let restore_id = db
        .write(
            Principal::system(
                lis_ops::restore::RESTORE_COMPONENT,
                [Capability::ManageQuality],
            ),
            CorrelationId::generate(),
            move |ctx| {
                lis_ops::restore::record_applied_restore::<RestoreRecordError>(
                    ctx,
                    &marker,
                    |ctx| {
                        let floors = lis_specimens::raise_number_floors_after_restore(
                            ctx,
                            lis_ops::policy::RESTORE_ACCESSION_SKIP,
                            lis_ops::policy::RESTORE_CONTAINER_SKIP,
                        )?;
                        Ok(RestoreFloors {
                            accession: floor_of(floors.accession),
                            container: floor_of(floors.container),
                        })
                    },
                )
            },
        )
        .await
        .map_err(|error| failed(&error))?;
    tracing::info!(%restore_id, "restore recorded; recovery reconciliation open");
    if let Err(error) = lis_ops::restore::finish_recorded_restore(data_dir) {
        tracing::warn!(%error, "restore staging folder not removed");
    }
    Ok(())
}

fn floor_of(position: lis_specimens::NumberPosition) -> String {
    position.floor.unwrap_or(position.next)
}

/// Moves every published backup set that no successful copy records to its target's
/// quarantine folder (REQ-OPS-007: a power loss between the set's final rename and its
/// copy row), before interrupted runs are marked and before any set is listed or pruned.
/// Without a usable installation key no manifest can be authenticated and backups cannot
/// run, so it waits for a start with the key. A failure is logged and the start goes on:
/// an unreadable backup folder must not keep the laboratory out of its own system.
pub(crate) async fn reconcile_published_sets(db: &Database, key: Option<DataKey>) {
    let Some(key) = key else {
        tracing::warn!("installation key not usable; published backup sets not reconciled");
        return;
    };
    let key = BackupKey::derive(&key);
    let reconciled = db
        .read(move |conn| lis_ops::backup::reconcile_published_sets(conn, &key))
        .await;
    match reconciled {
        Ok(reconciled) => tracing::info!(
            quarantined = reconciled.quarantined,
            unreadable = reconciled.unreadable,
            "published backup sets reconciled"
        ),
        Err(error) => tracing::error!(%error, "published backup sets not reconciled"),
    }
}

/// Records the app run and marks interrupted jobs (§6.4 step 8).
pub(crate) async fn start_run(
    db: &Database,
    webview: Option<String>,
) -> Result<AppRunId, EngineError> {
    let start = RunStart {
        app_version: APP_VERSION.to_owned(),
        os_text: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        webview_text: webview,
    };
    let started = lifecycle_write(db, move |ctx| lis_ops::runs::start_run(ctx, &start)).await?;
    let interrupted = lifecycle_write(db, lis_ops::backup::interrupt_abandoned).await?;
    tracing::info!(
        run_id = %started.run_id,
        previous_unclean = started.previous_unclean,
        gap_seconds = started.gap_seconds,
        version_changed = started.version_changed,
        webview_changed = started.webview_changed,
        interrupted_jobs = interrupted,
        "app run started"
    );
    Ok(started.run_id)
}
