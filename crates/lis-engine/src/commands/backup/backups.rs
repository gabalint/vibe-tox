//! Backup-target, run, set and restore-test commands.

use std::path::PathBuf;

use lis_core::{CommandError, UtcTimestamp};
use lis_ops::anchors::AnchorSummary;
use lis_ops::backup::{BackupError, RunKind, TargetRecord};
use lis_ops::restore::RestoreTestId;
use lis_ops::targets::{NewTarget, TargetId, TargetKind, TargetPurpose};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::views::{restore_test_view, run_view, set_view, target_available, target_view};
use super::{
    ADD_BACKUP_TARGET, BackupRunView, BackupSetView, BackupStatus, FindingView, GET_BACKUP_STATUS,
    LIST_BACKUP_RUNS, LIST_BACKUP_SETS, LIST_BACKUP_TARGETS, LIST_RESTORE_TESTS,
    NewestVerifiedView, REQUEST_BACKUP, REQUEST_RESTORE_TEST, RETIRE_BACKUP_TARGET, TargetView,
    checked_limit,
};
use crate::engine::Engine;

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddBackupTargetInput {
    pub label: String,
    pub kind: TargetKind,
    pub purpose: TargetPurpose,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RetireBackupTargetInput {
    pub target_id: TargetId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RequestBackupInput {
    pub kind: RunKind,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListBackupRunsInput {
    pub limit: u32,
    pub before_requested_at: Option<UtcTimestamp>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub before_run_id: Option<lis_ops::backup::RunId>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub run_id: Option<lis_ops::backup::RunId>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListRestoreTestsInput {
    pub limit: u32,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub before_test_id: Option<RestoreTestId>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub test_id: Option<RestoreTestId>,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TargetInput {
    pub target_id: TargetId,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RequestRestoreTestInput {
    pub target_id: TargetId,
    pub set_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreTestView {
    pub test_id: RestoreTestId,
    pub target_label: String,
    pub set_id: Option<String>,
    pub set_created_at: Option<UtcTimestamp>,
    pub state: String,
    pub requested_by: String,
    pub requested_at: UtcTimestamp,
    pub started_at: Option<UtcTimestamp>,
    pub finished_at: Option<UtcTimestamp>,
    #[ts(type = "number | null")]
    pub elapsed_ms: Option<u64>,
    pub failure_code: Option<String>,
    pub anchors: Option<AnchorSummary>,
    pub findings: Vec<FindingView>,
    pub passed: bool,
}

impl Engine {
    pub async fn list_backup_targets(&self) -> Result<Vec<TargetView>, CommandError> {
        self.run(&LIST_BACKUP_TARGETS, |call| async move {
            self.read(&call, |conn| {
                lis_ops::backup::list_targets(conn)
                    .map(|records| records.into_iter().map(target_view).collect())
            })
            .await
        })
        .await
    }

    pub async fn add_backup_target(
        &self,
        input: AddBackupTargetInput,
        folder: Option<PathBuf>,
    ) -> Result<Option<TargetView>, CommandError> {
        self.run(&ADD_BACKUP_TARGET, |call| async move {
            // The capability is checked before a cancelled dialog is answered (NFR-31).
            self.authorize(&call).await?;
            let Some(folder) = folder else {
                return Ok(None);
            };
            let data_dir = self.ops.data_dir.clone();
            let key = self.data_key().await?;
            let key_id = lis_ops::format::BackupKey::derive(&key).key_id();
            let target = self
                .write(&call, move |ctx| {
                    lis_ops::targets::add_target(
                        ctx,
                        &data_dir,
                        key_id,
                        &NewTarget {
                            label: input.label,
                            kind: input.kind,
                            purpose: input.purpose,
                            folder,
                        },
                    )
                })
                .await?;
            Ok(Some(target_view(TargetRecord {
                target,
                retired: false,
                last_verified_at: None,
                last_failure: None,
            })))
        })
        .await
    }

    pub async fn retire_backup_target(
        &self,
        input: RetireBackupTargetInput,
    ) -> Result<TargetView, CommandError> {
        self.run(&RETIRE_BACKUP_TARGET, |call| async move {
            let target_id = input.target_id;
            self.write(&call, move |ctx| {
                lis_ops::targets::retire_target(
                    ctx,
                    target_id,
                    input.expected_revision,
                    &input.reason,
                )?;
                lis_ops::backup::list_targets(ctx.tx())?
                    .into_iter()
                    .find(|record| record.target.id == target_id)
                    .map(target_view)
                    .ok_or(BackupError::NotFound("backup target"))
            })
            .await
        })
        .await
    }

    pub async fn get_backup_status(&self) -> Result<BackupStatus, CommandError> {
        self.run(&GET_BACKUP_STATUS, |call| async move {
            let now = self.clock.now();
            let settings = self
                .read(&call, |conn| {
                    lis_ops::settings::current(conn).map_err(|error| match error {
                        lis_ops::RuntimeError::Store(error) => BackupError::Store(error),
                        _ => BackupError::NoInstallation,
                    })
                })
                .await?;
            self.read(&call, move |conn| {
                let available: Vec<_> = lis_ops::backup::list_targets(conn)?
                    .into_iter()
                    .filter(target_available)
                    .map(|record| record.target.id)
                    .collect();
                let status = lis_ops::backup::status(
                    conn,
                    now,
                    settings.backup_interval_minutes,
                    &available,
                )?;
                Ok::<_, BackupError>(BackupStatus {
                    next_due_at: status.next_due_at,
                    running: status.running.map(|run| run_view(run, now)),
                    newest_verified: status
                        .newest_verified
                        .map(|(at, target_label)| NewestVerifiedView { at, target_label }),
                    up_to_date: status.up_to_date,
                })
            })
            .await
        })
        .await
    }

    pub async fn request_backup(
        &self,
        input: RequestBackupInput,
    ) -> Result<BackupRunView, CommandError> {
        self.run(&REQUEST_BACKUP, |call| async move {
            let run = self
                .write(&call, move |ctx| {
                    let id = lis_ops::backup::request_backup(ctx, input.kind)?;
                    lis_ops::backup::get_run(ctx.tx(), id).map(|run| run_view(run, ctx.now()))
                })
                .await?;
            self.wake_job_runner();
            Ok(run)
        })
        .await
    }

    pub async fn list_backup_runs(
        &self,
        input: ListBackupRunsInput,
    ) -> Result<Vec<BackupRunView>, CommandError> {
        self.run(&LIST_BACKUP_RUNS, |call| async move {
            let limit = checked_limit(input.limit)?;
            let now = self.clock.now();
            self.read(&call, move |conn| {
                select_backup_runs(conn, input, limit, now)
            })
            .await
        })
        .await
    }

    pub async fn list_backup_sets(
        &self,
        input: TargetInput,
    ) -> Result<Vec<BackupSetView>, CommandError> {
        self.run(&LIST_BACKUP_SETS, |call| async move {
            let target = self
                .read(&call, move |conn| {
                    lis_ops::backup::list_targets(conn)?
                        .into_iter()
                        .find(|record| record.target.id == input.target_id && !record.retired)
                        .map(|record| record.target)
                        .ok_or(BackupError::NotFound("backup target"))
                })
                .await?;
            let key = lis_ops::format::BackupKey::derive(&self.data_key().await?);
            let listings = lis_ops::backup::list_sets(&lis_ops::targets::root(&target), &key)?;
            let (sets, _) = lis_ops::restore::summarize_sets(listings, crate::engine::MODULES);
            Ok(sets.into_iter().map(set_view).collect())
        })
        .await
    }

    pub async fn request_restore_test(
        &self,
        input: RequestRestoreTestInput,
    ) -> Result<RestoreTestView, CommandError> {
        self.run(&REQUEST_RESTORE_TEST, |call| async move {
            let test = self
                .write(&call, move |ctx| {
                    let id = lis_ops::restore::request_restore_test(
                        ctx,
                        input.target_id,
                        input.set_id.as_deref(),
                    )?;
                    lis_ops::restore::get_restore_test(ctx.tx(), id).map(restore_test_view)
                })
                .await?;
            self.wake_job_runner();
            Ok(test)
        })
        .await
    }

    pub async fn list_restore_tests(
        &self,
        input: ListRestoreTestsInput,
    ) -> Result<Vec<RestoreTestView>, CommandError> {
        self.run(&LIST_RESTORE_TESTS, |call| async move {
            let limit = checked_limit(input.limit)?;
            self.read(&call, move |conn| select_restore_tests(conn, input, limit))
                .await
        })
        .await
    }
}

fn select_backup_runs(
    conn: &rusqlite::Connection,
    input: ListBackupRunsInput,
    limit: u32,
    now: UtcTimestamp,
) -> Result<Vec<BackupRunView>, BackupError> {
    if input.before_run_id.is_some() && input.before_requested_at.is_some() {
        return Err(selection_conflict());
    }
    if let Some(id) = input.run_id {
        if input.before_run_id.is_some() || input.before_requested_at.is_some() {
            return Err(selection_conflict());
        }
        return lis_ops::backup::get_run(conn, id).map(|run| vec![run_view(run, now)]);
    }
    lis_ops::backup::list_runs_page(conn, limit, input.before_requested_at, input.before_run_id)
        .map(|runs| runs.into_iter().map(|run| run_view(run, now)).collect())
}

fn select_restore_tests(
    conn: &rusqlite::Connection,
    input: ListRestoreTestsInput,
    limit: u32,
) -> Result<Vec<RestoreTestView>, BackupError> {
    if let Some(id) = input.test_id {
        if input.before_test_id.is_some() {
            return Err(selection_conflict());
        }
        return lis_ops::restore::get_restore_test(conn, id)
            .map(|test| vec![restore_test_view(test)]);
    }
    lis_ops::restore::list_restore_tests_page(conn, limit, input.before_test_id)
        .map(|tests| tests.into_iter().map(restore_test_view).collect())
}

fn selection_conflict() -> BackupError {
    BackupError::Invalid(lis_core::FieldError {
        field: "selection".to_owned(),
        code: "conflicting_selection".to_owned(),
        message: "Choose an exact record or a history cursor, not both.".to_owned(),
    })
}
