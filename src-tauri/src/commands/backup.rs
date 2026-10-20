//! Thin backup, restore, recovery-kit and exit-export adapters (REQ-OPS-002/003/007/010).
//! Native dialogs resolve every path in Rust; the UI never receives or supplies a path. A
//! dialog opens only after the engine's preflight of its command (M8).

use std::path::PathBuf;

use lis_core::CommandError;
use lis_engine::{
    AddBackupTargetInput, AnchorSummary, BackupRunView, BackupSetView, BackupStatus,
    BeginRestoreInput, CloseRecoveryInput, ConfirmRecoveryKitStorageInput, ConfirmRestoreInput,
    ConfirmRestoreView, DialogCommand, Engine, ExitExportView, ExportRecoveryKitInput, ExportedKit,
    LimitInput, ListBackupRunsInput, NumberFloors, RaiseNumberFloorsInput, RecoveryKitStatus,
    RecoveryReconciliation, RequestBackupInput, RequestExitExportInput, RequestRestoreTestInput,
    RestoreAvailability, RestoreSessionInput, RestoreSessionView, RestoreSourceView,
    RestoreTestView, RetireBackupTargetInput, StageRestoreInput, StagedRestoreView, TargetInput,
    TargetView,
};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) async fn list_backup_targets(
    engine: State<'_, Engine>,
) -> Result<Vec<TargetView>, CommandError> {
    engine.list_backup_targets().await
}

#[tauri::command]
pub(crate) async fn add_backup_target<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: AddBackupTargetInput,
) -> Result<Option<TargetView>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::AddBackupTarget)
        .await?;
    engine
        .add_backup_target(input, pick_folder(app).await)
        .await
}

#[tauri::command]
pub(crate) async fn retire_backup_target(
    engine: State<'_, Engine>,
    input: RetireBackupTargetInput,
) -> Result<TargetView, CommandError> {
    engine.retire_backup_target(input).await
}

#[tauri::command]
pub(crate) async fn get_backup_status(
    engine: State<'_, Engine>,
) -> Result<BackupStatus, CommandError> {
    engine.get_backup_status().await
}

#[tauri::command]
pub(crate) async fn request_backup(
    engine: State<'_, Engine>,
    input: RequestBackupInput,
) -> Result<BackupRunView, CommandError> {
    engine.request_backup(input).await
}

#[tauri::command]
pub(crate) async fn list_backup_runs(
    engine: State<'_, Engine>,
    input: ListBackupRunsInput,
) -> Result<Vec<BackupRunView>, CommandError> {
    engine.list_backup_runs(input).await
}

#[tauri::command]
pub(crate) async fn list_backup_sets(
    engine: State<'_, Engine>,
    input: TargetInput,
) -> Result<Vec<BackupSetView>, CommandError> {
    engine.list_backup_sets(input).await
}

#[tauri::command]
pub(crate) async fn request_restore_test(
    engine: State<'_, Engine>,
    input: RequestRestoreTestInput,
) -> Result<RestoreTestView, CommandError> {
    engine.request_restore_test(input).await
}

#[tauri::command]
pub(crate) async fn list_restore_tests(
    engine: State<'_, Engine>,
    input: lis_engine::ListRestoreTestsInput,
) -> Result<Vec<RestoreTestView>, CommandError> {
    engine.list_restore_tests(input).await
}

#[tauri::command]
pub(crate) async fn get_recovery_kit_status(
    engine: State<'_, Engine>,
) -> Result<RecoveryKitStatus, CommandError> {
    engine.get_recovery_kit_status().await
}

#[tauri::command]
pub(crate) async fn export_recovery_kit<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: ExportRecoveryKitInput,
) -> Result<Option<ExportedKit>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::ExportRecoveryKit)
        .await?;
    let destination = save_file(app, "vibe-tox-recovery-kit.txt").await;
    engine.export_recovery_kit(input, destination).await
}

#[tauri::command]
pub(crate) async fn confirm_recovery_kit_storage(
    engine: State<'_, Engine>,
    input: ConfirmRecoveryKitStorageInput,
) -> Result<RecoveryKitStatus, CommandError> {
    engine.confirm_recovery_kit_storage(input).await
}

#[tauri::command]
pub(crate) async fn get_restore_availability(
    engine: State<'_, Engine>,
) -> Result<RestoreAvailability, CommandError> {
    engine.get_restore_availability().await
}

#[tauri::command]
pub(crate) async fn begin_restore(
    engine: State<'_, Engine>,
    input: BeginRestoreInput,
) -> Result<RestoreSessionView, CommandError> {
    engine.begin_restore(input).await
}

#[tauri::command]
pub(crate) async fn choose_restore_source<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: RestoreSessionInput,
) -> Result<Option<RestoreSourceView>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::ChooseRestoreSource(input.restore_session_id))
        .await?;
    engine
        .choose_restore_source(input, pick_folder(app).await)
        .await
}

#[tauri::command]
pub(crate) async fn stage_restore(
    engine: State<'_, Engine>,
    input: StageRestoreInput,
) -> Result<StagedRestoreView, CommandError> {
    engine.stage_restore(input).await
}

#[tauri::command]
pub(crate) async fn check_restore_anchors<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: RestoreSessionInput,
) -> Result<Option<AnchorSummary>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::CheckRestoreAnchors(input.restore_session_id))
        .await?;
    engine
        .check_restore_anchors(input, pick_folder(app).await)
        .await
}

#[tauri::command]
pub(crate) async fn confirm_restore<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: ConfirmRestoreInput,
) -> Result<ConfirmRestoreView, CommandError> {
    let result = engine.confirm_restore(input).await?;
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        app.exit(0);
    });
    Ok(result)
}

#[tauri::command]
pub(crate) async fn cancel_restore(
    engine: State<'_, Engine>,
    input: RestoreSessionInput,
) -> Result<(), CommandError> {
    engine.cancel_restore(input).await
}

#[tauri::command]
pub(crate) async fn get_recovery_reconciliation(
    engine: State<'_, Engine>,
) -> Result<Option<RecoveryReconciliation>, CommandError> {
    engine.get_recovery_reconciliation().await
}

#[tauri::command]
pub(crate) async fn close_recovery_reconciliation(
    engine: State<'_, Engine>,
    input: CloseRecoveryInput,
) -> Result<(), CommandError> {
    engine.close_recovery_reconciliation(input).await
}

#[tauri::command]
pub(crate) async fn raise_number_floors(
    engine: State<'_, Engine>,
    input: RaiseNumberFloorsInput,
) -> Result<NumberFloors, CommandError> {
    engine.raise_number_floors(input).await
}

#[tauri::command]
pub(crate) async fn request_exit_export<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: RequestExitExportInput,
) -> Result<Option<ExitExportView>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::RequestExitExport)
        .await?;
    engine
        .request_exit_export(input, pick_folder(app).await)
        .await
}

#[tauri::command]
pub(crate) async fn list_exit_exports(
    engine: State<'_, Engine>,
    input: LimitInput,
) -> Result<Vec<ExitExportView>, CommandError> {
    engine.list_exit_exports(input).await
}

pub(super) async fn pick_folder<R: Runtime>(app: AppHandle<R>) -> Option<PathBuf> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await;
    match picked {
        Ok(path) => path.and_then(|path| path.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, dialog = "folder", "native dialog stopped");
            None
        }
    }
}

pub(super) async fn save_file<R: Runtime>(
    app: AppHandle<R>,
    file_name: &'static str,
) -> Option<PathBuf> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name(file_name)
            .blocking_save_file()
    })
    .await;
    match picked {
        Ok(path) => path.and_then(|path| path.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, dialog = "save", "native dialog stopped");
            None
        }
    }
}
