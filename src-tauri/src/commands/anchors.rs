//! Thin audit anchor adapters (NFR-18, ADR 0008; M4 contract "Audit anchors"). The
//! folder dialog of `verify_audit_anchors` opens here, in Rust; the UI never sends a path
//! and no `dialog:` permission is granted (M4 D10). The dialog opens only after the
//! engine's preflight (M8). A cancelled dialog returns `null`.

use std::path::PathBuf;

use lis_core::CommandError;
use lis_engine::{
    AnchorExportReport, AnchorExportView, AnchorStatusView, AnchorVerification, DialogCommand,
    Engine, ListAnchorExportsInput, VerifyAuditAnchorsInput,
};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) async fn get_anchor_status(
    engine: State<'_, Engine>,
) -> Result<AnchorStatusView, CommandError> {
    engine.get_anchor_status().await
}

#[tauri::command]
pub(crate) async fn list_anchor_exports(
    engine: State<'_, Engine>,
    input: ListAnchorExportsInput,
) -> Result<Vec<AnchorExportView>, CommandError> {
    engine.list_anchor_exports(input).await
}

#[tauri::command]
pub(crate) async fn export_anchors_now(
    engine: State<'_, Engine>,
) -> Result<AnchorExportReport, CommandError> {
    engine.export_anchors_now().await
}

#[tauri::command]
pub(crate) async fn verify_audit_anchors<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: VerifyAuditAnchorsInput,
) -> Result<Option<AnchorVerification>, CommandError> {
    let folder = if input.choose_folder {
        engine
            .preflight_dialog(DialogCommand::VerifyAuditAnchors)
            .await?;
        let Some(folder) = pick_folder(app).await else {
            return Ok(None);
        };
        Some(folder)
    } else {
        None
    };
    engine.verify_audit_anchors(folder).await.map(Some)
}

/// The folder chosen in a native dialog, off the async runtime; `None` when cancelled.
async fn pick_folder<R: Runtime>(app: AppHandle<R>) -> Option<PathBuf> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await;
    match picked {
        Ok(folder) => folder.and_then(|folder| folder.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, "folder dialog stopped");
            None
        }
    }
}
