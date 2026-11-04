//! Interfaces: connections to outside systems, the message log and Unrouted messages
//! (capability `manage_configuration`). The folder dialog opens here, in Rust, after the
//! engine's preflight; the UI never sends a path and no `dialog:` permission is granted.

use lis_core::CommandError;
use lis_engine::{
    ApprovePartnerMappings, ChosenPartnerFolder, CreatePartnerConnectionInput, DialogCommand,
    DiscardPartnerMappingDraft, DraftPartnerMapping, Engine, ExportedMessageLog, FolderTestResult,
    ListPartnerMessages, PartnerConnection, PartnerConnectionDetail, PartnerConnectionInput,
    PartnerMappings, PartnerMessagePage, RecordPartnerDestinationCheck, ResendPartnerDelivery,
    ResolveUnroutedMessage, SetPartnerConnectionActive, TestPartnerFolderInput,
    TestPartnerSftpInput, UnroutedMessage, UpdatePartnerConnectionInput,
};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) async fn list_partner_connections(
    engine: State<'_, Engine>,
) -> Result<Vec<PartnerConnection>, CommandError> {
    engine.list_partner_connections().await
}

#[tauri::command]
pub(crate) async fn get_partner_connection(
    engine: State<'_, Engine>,
    input: PartnerConnectionInput,
) -> Result<PartnerConnectionDetail, CommandError> {
    engine.get_partner_connection(input).await
}

#[tauri::command]
pub(crate) async fn choose_partner_folder<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
) -> Result<Option<ChosenPartnerFolder>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::ChoosePartnerFolder)
        .await?;
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await;
    let folder = match picked {
        Ok(folder) => folder.and_then(|folder| folder.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, dialog = "folder", "native dialog stopped");
            None
        }
    };
    engine.choose_partner_folder(folder).await
}

#[tauri::command]
pub(crate) async fn test_partner_folder(
    engine: State<'_, Engine>,
    input: TestPartnerFolderInput,
) -> Result<FolderTestResult, CommandError> {
    engine.test_partner_folder(input).await
}

#[tauri::command]
pub(crate) async fn test_partner_sftp(
    engine: State<'_, Engine>,
    input: TestPartnerSftpInput,
) -> Result<FolderTestResult, CommandError> {
    engine.test_partner_sftp(input).await
}

#[tauri::command]
pub(crate) async fn get_lis_sftp_key(engine: State<'_, Engine>) -> Result<String, CommandError> {
    engine.get_lis_sftp_key().await
}

#[tauri::command]
pub(crate) async fn create_partner_connection(
    engine: State<'_, Engine>,
    input: CreatePartnerConnectionInput,
) -> Result<PartnerConnectionDetail, CommandError> {
    engine.create_partner_connection(input).await
}

#[tauri::command]
pub(crate) async fn update_partner_connection(
    engine: State<'_, Engine>,
    input: UpdatePartnerConnectionInput,
) -> Result<PartnerConnectionDetail, CommandError> {
    engine.update_partner_connection(input).await
}

#[tauri::command]
pub(crate) async fn set_partner_connection_active(
    engine: State<'_, Engine>,
    input: SetPartnerConnectionActive,
) -> Result<PartnerConnectionDetail, CommandError> {
    engine.set_partner_connection_active(input).await
}

#[tauri::command]
pub(crate) async fn record_partner_destination_check(
    engine: State<'_, Engine>,
    input: RecordPartnerDestinationCheck,
) -> Result<PartnerConnectionDetail, CommandError> {
    engine.record_partner_destination_check(input).await
}

#[tauri::command]
pub(crate) async fn send_partner_test_message(
    engine: State<'_, Engine>,
    input: PartnerConnectionInput,
) -> Result<FolderTestResult, CommandError> {
    engine.send_partner_test_message(input).await
}

#[tauri::command]
pub(crate) async fn list_partner_messages(
    engine: State<'_, Engine>,
    input: ListPartnerMessages,
) -> Result<PartnerMessagePage, CommandError> {
    engine.list_partner_messages(input).await
}

#[tauri::command]
pub(crate) async fn export_partner_messages<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: ListPartnerMessages,
) -> Result<ExportedMessageLog, CommandError> {
    engine
        .export_partner_messages(input, app.path().download_dir().ok())
        .await
}

#[tauri::command]
pub(crate) async fn list_unrouted_partner_messages(
    engine: State<'_, Engine>,
) -> Result<Vec<UnroutedMessage>, CommandError> {
    engine.list_unrouted_partner_messages().await
}

#[tauri::command]
pub(crate) async fn resolve_unrouted_message(
    engine: State<'_, Engine>,
    input: ResolveUnroutedMessage,
) -> Result<(), CommandError> {
    engine.resolve_unrouted_message(input).await
}

#[tauri::command]
pub(crate) async fn resend_partner_delivery(
    engine: State<'_, Engine>,
    input: ResendPartnerDelivery,
) -> Result<(), CommandError> {
    engine.resend_partner_delivery(input).await
}

#[tauri::command]
pub(crate) async fn list_partner_mappings(
    engine: State<'_, Engine>,
    input: PartnerConnectionInput,
) -> Result<PartnerMappings, CommandError> {
    engine.list_partner_mappings(input).await
}

#[tauri::command]
pub(crate) async fn draft_partner_mapping(
    engine: State<'_, Engine>,
    input: DraftPartnerMapping,
) -> Result<PartnerMappings, CommandError> {
    engine.draft_partner_mapping(input).await
}

#[tauri::command]
pub(crate) async fn discard_partner_mapping_draft(
    engine: State<'_, Engine>,
    input: DiscardPartnerMappingDraft,
) -> Result<PartnerMappings, CommandError> {
    engine.discard_partner_mapping_draft(input).await
}

#[tauri::command]
pub(crate) async fn approve_partner_mappings(
    engine: State<'_, Engine>,
    input: ApprovePartnerMappings,
) -> Result<PartnerMappings, CommandError> {
    engine.approve_partner_mappings(input).await
}
