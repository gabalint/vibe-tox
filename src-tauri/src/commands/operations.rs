//! Thin runtime operations adapters (REQ-OPS-002, 003, 006, 008, 013; NFR-19): health and
//! its history, app runs, settings, the security and separation-of-duties policies, update
//! records, media disposals and the installation key. The update package is chosen in a
//! native file dialog in Rust, opened only after the engine's preflight (M8); the UI never
//! sends a path.

use std::path::PathBuf;

use lis_core::CommandError;
use lis_engine::{
    AcceptInstalledVersionInput, AcknowledgeListeningGapInput, AppRunView, AttestHealthCheckInput,
    DialogCommand, Engine, HealthCheckView, HealthEventView, HostHealthView,
    InstallationKeyStateView, InstalledVersionView, ListAppRunsInput, ListHealthHistoryInput,
    MediaDisposalView, OperationsSettingsView, RecordMediaDisposalInput, RecordUpdateApprovalInput,
    RestoreInstallationKeyInput, SecurityPolicyView, SeparationPolicyView,
    SetOperationsSettingsInput, SetSecurityPolicyInput, SetSeparationPolicyInput,
    UpdateApprovalView, WithdrawUpdateApprovalInput,
};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) async fn get_host_health(
    engine: State<'_, Engine>,
) -> Result<HostHealthView, CommandError> {
    engine.get_host_health().await
}

#[tauri::command]
pub(crate) async fn refresh_host_health(
    engine: State<'_, Engine>,
) -> Result<HostHealthView, CommandError> {
    engine.refresh_host_health().await
}

#[tauri::command]
pub(crate) async fn list_health_history(
    engine: State<'_, Engine>,
    input: ListHealthHistoryInput,
) -> Result<Vec<HealthEventView>, CommandError> {
    engine.list_health_history(input).await
}

#[tauri::command]
pub(crate) async fn attest_health_check(
    engine: State<'_, Engine>,
    input: AttestHealthCheckInput,
) -> Result<HealthCheckView, CommandError> {
    engine.attest_health_check(input).await
}

#[tauri::command]
pub(crate) async fn list_app_runs(
    engine: State<'_, Engine>,
    input: ListAppRunsInput,
) -> Result<Vec<AppRunView>, CommandError> {
    engine.list_app_runs(input).await
}

#[tauri::command]
pub(crate) async fn acknowledge_listening_gap(
    engine: State<'_, Engine>,
    input: AcknowledgeListeningGapInput,
) -> Result<AppRunView, CommandError> {
    engine.acknowledge_listening_gap(input).await
}

#[tauri::command]
pub(crate) async fn get_operations_settings(
    engine: State<'_, Engine>,
) -> Result<OperationsSettingsView, CommandError> {
    engine.get_operations_settings().await
}

#[tauri::command]
pub(crate) async fn set_operations_settings(
    engine: State<'_, Engine>,
    input: SetOperationsSettingsInput,
) -> Result<OperationsSettingsView, CommandError> {
    engine.set_operations_settings(input).await
}

#[tauri::command]
pub(crate) async fn get_security_policy(
    engine: State<'_, Engine>,
) -> Result<SecurityPolicyView, CommandError> {
    engine.get_security_policy().await
}

#[tauri::command]
pub(crate) async fn set_security_policy(
    engine: State<'_, Engine>,
    input: SetSecurityPolicyInput,
) -> Result<SecurityPolicyView, CommandError> {
    engine.set_security_policy(input).await
}

#[tauri::command]
pub(crate) async fn get_separation_policy(
    engine: State<'_, Engine>,
) -> Result<SeparationPolicyView, CommandError> {
    engine.get_separation_policy().await
}

#[tauri::command]
pub(crate) async fn set_separation_policy(
    engine: State<'_, Engine>,
    input: SetSeparationPolicyInput,
) -> Result<SeparationPolicyView, CommandError> {
    engine.set_separation_policy(input).await
}

#[tauri::command]
pub(crate) async fn list_installed_versions(
    engine: State<'_, Engine>,
) -> Result<Vec<InstalledVersionView>, CommandError> {
    engine.list_installed_versions().await
}

#[tauri::command]
pub(crate) async fn list_update_approvals(
    engine: State<'_, Engine>,
) -> Result<Vec<UpdateApprovalView>, CommandError> {
    engine.list_update_approvals().await
}

/// Opens the file dialog for the update package; `null` when it is cancelled.
#[tauri::command]
pub(crate) async fn record_update_approval<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: RecordUpdateApprovalInput,
) -> Result<Option<UpdateApprovalView>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::RecordUpdateApproval)
        .await?;
    engine
        .record_update_approval(input, pick_file(app).await)
        .await
}

#[tauri::command]
pub(crate) async fn withdraw_update_approval(
    engine: State<'_, Engine>,
    input: WithdrawUpdateApprovalInput,
) -> Result<UpdateApprovalView, CommandError> {
    engine.withdraw_update_approval(input).await
}

#[tauri::command]
pub(crate) async fn accept_installed_version(
    engine: State<'_, Engine>,
    input: AcceptInstalledVersionInput,
) -> Result<InstalledVersionView, CommandError> {
    engine.accept_installed_version(input).await
}

#[tauri::command]
pub(crate) async fn record_media_disposal(
    engine: State<'_, Engine>,
    input: RecordMediaDisposalInput,
) -> Result<MediaDisposalView, CommandError> {
    engine.record_media_disposal(input).await
}

#[tauri::command]
pub(crate) async fn list_media_disposals(
    engine: State<'_, Engine>,
) -> Result<Vec<MediaDisposalView>, CommandError> {
    engine.list_media_disposals().await
}

#[tauri::command]
pub(crate) async fn get_installation_key_state(
    engine: State<'_, Engine>,
) -> Result<InstallationKeyStateView, CommandError> {
    engine.get_installation_key_state().await
}

#[tauri::command]
pub(crate) async fn restore_installation_key(
    engine: State<'_, Engine>,
    input: RestoreInstallationKeyInput,
) -> Result<InstallationKeyStateView, CommandError> {
    engine.restore_installation_key(input).await
}

/// The file the user chose, off the main thread; `None` when cancelled.
pub(super) async fn pick_file<R: Runtime>(app: AppHandle<R>) -> Option<PathBuf> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_file())
            .await;
    match picked {
        Ok(path) => path.and_then(|path| path.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, dialog = "file", "native dialog stopped");
            None
        }
    }
}
