//! Backup, recovery-kit, restore, recovery reconciliation and exit-export commands
//! (REQ-OPS-002/003/007/010; NFR-04/12/30). File I/O runs outside write transactions;
//! paths come only from native desktop dialogs.

mod backups;
mod exit_export;
mod kit;
mod reconciliation;
mod restore;
#[cfg(test)]
mod tests;
mod views;

pub use backups::{
    AddBackupTargetInput, ListBackupRunsInput, ListRestoreTestsInput, RequestBackupInput,
    RequestRestoreTestInput, RestoreTestView, RetireBackupTargetInput, TargetInput,
};
pub use exit_export::{ExitExportView, RequestExitExportInput};
pub use kit::{
    ConfirmRecoveryKitStorageInput, ExportRecoveryKitInput, ExportedKit, RecoveryKitStatus,
};
pub use reconciliation::{CloseRecoveryInput, RecoveryReconciliation};
pub use restore::{
    BeginRestoreInput, ConfirmRestoreInput, ConfirmRestoreView, RestoreAvailability,
    RestoreSessionInput, RestoreSessionView, RestoreSourceView, StageRestoreInput,
    StagedRestoreView,
};
pub(crate) use restore::{RestoreStep, preflight_restore_step};

use lis_core::{Capability, FieldError, UtcTimestamp};
use lis_ops::backup::BackupError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::pipeline::domain_error;
use crate::registry::{Access, CommandSpec, spec};

const BACKUP: Access = Access::Capability(Capability::BackupRestore);
const QUALITY: Access = Access::Capability(Capability::ManageQuality);
const EXPORT: Access = Access::Capability(Capability::Export);
const MAX_LIST: u32 = 500;

pub(crate) const LIST_BACKUP_TARGETS: CommandSpec =
    spec("list_backup_targets", "ops.list_targets", BACKUP);
pub(crate) const ADD_BACKUP_TARGET: CommandSpec =
    spec("add_backup_target", lis_ops::targets::ADD_TARGET, BACKUP);
pub(crate) const RETIRE_BACKUP_TARGET: CommandSpec = spec(
    "retire_backup_target",
    lis_ops::targets::RETIRE_TARGET,
    BACKUP,
);
pub(crate) const GET_BACKUP_STATUS: CommandSpec =
    spec("get_backup_status", "ops.backup_status", BACKUP);
pub(crate) const REQUEST_BACKUP: CommandSpec =
    spec("request_backup", lis_ops::backup::REQUEST_BACKUP, BACKUP);
pub(crate) const LIST_BACKUP_RUNS: CommandSpec =
    spec("list_backup_runs", "ops.list_backup_runs", BACKUP);
pub(crate) const LIST_BACKUP_SETS: CommandSpec =
    spec("list_backup_sets", "ops.list_backup_sets", BACKUP);
pub(crate) const REQUEST_RESTORE_TEST: CommandSpec = spec(
    "request_restore_test",
    lis_ops::restore::REQUEST_RESTORE_TEST,
    BACKUP,
);
pub(crate) const LIST_RESTORE_TESTS: CommandSpec =
    spec("list_restore_tests", "ops.list_restore_tests", BACKUP);
pub(crate) const GET_RECOVERY_KIT_STATUS: CommandSpec =
    spec("get_recovery_kit_status", "ops.recovery_kit_status", BACKUP);
pub(crate) const EXPORT_RECOVERY_KIT: CommandSpec = spec(
    "export_recovery_kit",
    lis_ops::kit::EXPORT_RECOVERY_KIT,
    BACKUP,
);
pub(crate) const CONFIRM_RECOVERY_KIT_STORAGE: CommandSpec = spec(
    "confirm_recovery_kit_storage",
    lis_ops::kit::CONFIRM_RECOVERY_KIT,
    BACKUP,
);
pub(crate) const GET_RESTORE_AVAILABILITY: CommandSpec = spec(
    "get_restore_availability",
    "ops.restore_availability",
    Access::Public,
);
pub(crate) const BEGIN_RESTORE: CommandSpec =
    spec("begin_restore", "ops.begin_restore", Access::Public);
pub(crate) const CHOOSE_RESTORE_SOURCE: CommandSpec = spec(
    "choose_restore_source",
    "ops.choose_restore_source",
    Access::Public,
);
pub(crate) const STAGE_RESTORE: CommandSpec =
    spec("stage_restore", "ops.stage_restore", Access::Public);
pub(crate) const CHECK_RESTORE_ANCHORS: CommandSpec = spec(
    "check_restore_anchors",
    "ops.check_restore_anchors",
    Access::Public,
);
pub(crate) const CONFIRM_RESTORE: CommandSpec =
    spec("confirm_restore", "ops.confirm_restore", Access::Public);
pub(crate) const CANCEL_RESTORE: CommandSpec =
    spec("cancel_restore", "ops.cancel_restore", Access::Public);
pub(crate) const GET_RECOVERY_RECONCILIATION: CommandSpec = spec(
    "get_recovery_reconciliation",
    "ops.get_recovery",
    Access::Session,
);
pub(crate) const CLOSE_RECOVERY_RECONCILIATION: CommandSpec = spec(
    "close_recovery_reconciliation",
    lis_ops::recovery::CLOSE_RECOVERY,
    QUALITY,
);
pub(crate) const RAISE_NUMBER_FLOORS: CommandSpec = spec(
    "raise_number_floors",
    lis_specimens::actions::RAISE_NUMBER_FLOOR,
    QUALITY,
);
pub(crate) const REQUEST_EXIT_EXPORT: CommandSpec = spec(
    "request_exit_export",
    lis_ops::exit_export::REQUEST_EXIT_EXPORT,
    EXPORT,
);
pub(crate) const LIST_EXIT_EXPORTS: CommandSpec =
    spec("list_exit_exports", "ops.list_exit_exports", EXPORT);

domain_error!(BackupError, |error: &BackupError| match error {
    BackupError::Invalid(field) => vec![field.clone()],
    _ => Vec::new(),
});

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TargetView {
    pub target_id: lis_ops::targets::TargetId,
    pub label: String,
    pub kind: lis_ops::targets::TargetKind,
    pub purpose: lis_ops::targets::TargetPurpose,
    pub location: String,
    pub available: bool,
    pub last_verified_at: Option<UtcTimestamp>,
    pub last_failure: Option<TargetFailureView>,
    pub retired: bool,
    #[ts(type = "number")]
    pub revision: i64,
    pub allowed_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TargetFailureView {
    pub code: String,
    pub at: UtcTimestamp,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CopyView {
    pub target_id: lis_ops::targets::TargetId,
    pub target_label: String,
    pub set_id: Option<String>,
    pub state: String,
    pub failure_code: Option<String>,
    pub verified_at: Option<UtcTimestamp>,
    pub pruned: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupRunView {
    pub run_id: lis_ops::backup::RunId,
    pub kind: String,
    pub state: String,
    pub requested_by: Option<String>,
    pub requested_at: UtcTimestamp,
    pub started_at: Option<UtcTimestamp>,
    pub finished_at: Option<UtcTimestamp>,
    #[ts(type = "number | null")]
    pub database_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub artifact_count: Option<u64>,
    #[ts(type = "number")]
    pub artifact_problems: u64,
    pub failure_code: Option<String>,
    pub copies: Vec<CopyView>,
    // Rechecked against the current clock when an approval is saved.
    pub update_approval_eligible: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupSetView {
    pub set_id: String,
    pub created_at: UtcTimestamp,
    pub kind: String,
    pub app_version: String,
    #[ts(type = "number")]
    pub database_bytes: u64,
    #[ts(type = "number")]
    pub artifact_count: usize,
    pub compatible: bool,
    pub incompatible_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupStatus {
    pub next_due_at: Option<UtcTimestamp>,
    pub running: Option<BackupRunView>,
    pub newest_verified: Option<NewestVerifiedView>,
    pub up_to_date: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewestVerifiedView {
    pub at: UtcTimestamp,
    pub target_label: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FindingView {
    pub code: String,
    #[ts(type = "number")]
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LimitInput {
    pub limit: u32,
}

fn checked_limit(limit: u32) -> Result<u32, BackupError> {
    if (1..=MAX_LIST).contains(&limit) {
        Ok(limit)
    } else {
        Err(BackupError::Invalid(FieldError {
            field: "limit".to_owned(),
            code: "out_of_range".to_owned(),
            message: format!("Choose 1 to {MAX_LIST} records."),
        }))
    }
}
