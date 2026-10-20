//! Thin file template adapters (result files brief §7, §9, §13). The sample-file dialog
//! opens here, in Rust, after the engine's preflight; the UI never sends a path and no
//! `dialog:` permission is granted. A cancelled dialog returns `null`.

use std::path::PathBuf;

use lis_core::CommandError;
use lis_engine::{
    CheckFileLayoutInput, DialogCommand, Engine, LayoutCheck, LayoutSampleOpened,
    PreviewLayoutSampleInput, PreviewLine, RegisterFileSourceInput, SourceRegistrationView,
};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) async fn open_layout_sample<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
) -> Result<Option<LayoutSampleOpened>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::OpenLayoutSample)
        .await?;
    engine.open_layout_sample(pick_sample(app).await).await
}

#[tauri::command]
pub(crate) async fn preview_layout_sample(
    engine: State<'_, Engine>,
    input: PreviewLayoutSampleInput,
) -> Result<Vec<PreviewLine>, CommandError> {
    engine.preview_layout_sample(input).await
}

#[tauri::command]
pub(crate) async fn check_file_layout(
    engine: State<'_, Engine>,
    input: CheckFileLayoutInput,
) -> Result<LayoutCheck, CommandError> {
    engine.check_file_layout(input).await
}

#[tauri::command]
pub(crate) async fn register_file_source(
    engine: State<'_, Engine>,
    input: RegisterFileSourceInput,
) -> Result<SourceRegistrationView, CommandError> {
    engine.register_file_source(input).await
}

/// A `.csv` or `.txt` file chosen in the native dialog, off the main thread.
async fn pick_sample<R: Runtime>(app: AppHandle<R>) -> Option<PathBuf> {
    let dialog = app
        .dialog()
        .file()
        .add_filter("Result files", &["csv", "txt"]);
    match tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await {
        Ok(path) => path.and_then(|path| path.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, dialog = "sample file", "native dialog stopped");
            None
        }
    }
}
