//! Native hardware transport wraps engine-owned authentication ceremonies.

use lis_auth::{AuthenticationStatus, SecurityKeyCredentialView};
use lis_core::CommandError;
use lis_engine::{
    DialogCommand, Engine, KeyRecoveryAcknowledgement, KeyRecoveryResult, KeySignInChoicesInput,
    RevokeSecurityKeyInput, SecurityKeyAction, SecurityKeyActionInput, SecurityKeyEnrollInput,
    SecurityKeyRecoveryInput, SecurityKeySignInInput, SecurityKeySignupInput,
    SecurityKeyUnlockInput, SessionInfo, UpdateRequest,
};
use serde_json::Value;
use tauri::{AppHandle, Runtime, State};

use crate::security_key_operations::{
    SecurityKeyOperationInput, SecurityKeyOperations, SecurityKeyPinInput, SecurityKeyStatus,
};
use crate::updater::{self, Updates};

#[tauri::command]
pub(crate) async fn security_key_signup(
    engine: State<'_, Engine>,
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeySignupInput,
) -> Result<KeyRecoveryResult, CommandError> {
    let client = operations.begin()?;
    engine.security_key_signup(input, &client).await
}

#[tauri::command]
pub(crate) async fn acknowledge_key_recovery(
    engine: State<'_, Engine>,
    input: KeyRecoveryAcknowledgement,
) -> Result<SessionInfo, CommandError> {
    engine.acknowledge_key_recovery(input).await
}

#[tauri::command]
pub(crate) async fn security_key_sign_in(
    engine: State<'_, Engine>,
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeySignInInput,
) -> Result<SessionInfo, CommandError> {
    let client = operations.begin()?;
    engine.security_key_sign_in(input, &client).await
}

#[tauri::command]
pub(crate) async fn security_key_unlock(
    engine: State<'_, Engine>,
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeyUnlockInput,
) -> Result<SessionInfo, CommandError> {
    let client = operations.begin()?;
    engine.security_key_unlock(input, &client).await
}

#[tauri::command]
pub(crate) async fn security_key_enroll(
    engine: State<'_, Engine>,
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeyEnrollInput,
) -> Result<KeyRecoveryResult, CommandError> {
    let client = operations.begin()?;
    engine.security_key_enroll(input, &client).await
}

#[tauri::command]
pub(crate) async fn security_key_recover(
    engine: State<'_, Engine>,
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeyRecoveryInput,
) -> Result<KeyRecoveryResult, CommandError> {
    let client = operations.begin()?;
    engine.security_key_recover(input, &client).await
}

#[tauri::command]
pub(crate) async fn get_authentication_status(
    engine: State<'_, Engine>,
) -> Result<AuthenticationStatus, CommandError> {
    engine.get_authentication_status().await
}

#[tauri::command]
pub(crate) async fn list_key_sign_in_choices(
    engine: State<'_, Engine>,
    input: KeySignInChoicesInput,
) -> Result<Vec<SecurityKeyCredentialView>, CommandError> {
    engine.list_key_sign_in_choices(input).await
}

#[tauri::command]
pub(crate) async fn revoke_security_key(
    engine: State<'_, Engine>,
    input: RevokeSecurityKeyInput,
) -> Result<(), CommandError> {
    engine.revoke_security_key(input).await
}

#[tauri::command]
pub(crate) async fn security_key_action<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    operations: State<'_, SecurityKeyOperations>,
    updates: State<'_, Updates>,
    input: SecurityKeyActionInput,
) -> Result<Value, CommandError> {
    // Reuse the password route's engine preflight and native dialogs. No path can
    // be supplied by the renderer. The engine rechecks gates at final dispatch.
    let destination = match &input.action {
        SecurityKeyAction::ExportRecoveryKit(_) => {
            engine
                .preflight_dialog(DialogCommand::ExportRecoveryKit)
                .await?;
            super::backup::save_file(app, "vibe-tox-recovery-kit.txt").await
        }
        SecurityKeyAction::RequestExitExport(_) => {
            engine
                .preflight_dialog(DialogCommand::RequestExitExport)
                .await?;
            super::backup::pick_folder(app).await
        }
        SecurityKeyAction::RecordUpdateApproval(_) => {
            engine
                .preflight_dialog(DialogCommand::RecordUpdateApproval)
                .await?;
            super::operations::pick_file(app).await
        }
        SecurityKeyAction::ApproveDownloadedUpdate(input) => {
            engine.preflight_update(UpdateRequest::Approve).await?;
            Some(updater::staged_path(&updates, &input.version)?)
        }
        _ => None,
    };
    let client = operations.begin()?;
    engine
        .security_key_action(input, destination, &client)
        .await
}

#[tauri::command]
pub(crate) fn security_key_status(
    operations: State<'_, SecurityKeyOperations>,
) -> Result<Option<SecurityKeyStatus>, CommandError> {
    operations.status()
}

#[tauri::command]
pub(crate) fn security_key_pin(
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeyPinInput,
) -> Result<(), CommandError> {
    operations.submit_pin(input)
}

#[tauri::command]
pub(crate) fn security_key_cancel(
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeyOperationInput,
) -> Result<(), CommandError> {
    operations.cancel(&input)
}

#[tauri::command]
pub(crate) fn security_key_continue(
    operations: State<'_, SecurityKeyOperations>,
    input: SecurityKeyOperationInput,
) -> Result<(), CommandError> {
    operations.continue_registration(&input)
}
