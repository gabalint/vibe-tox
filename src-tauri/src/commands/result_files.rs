//! Thin result-file adapters (REQ-INT-009; result-files brief §6, §9). The file and folder
//! dialogs open here, in Rust, after the engine's preflight; the UI never sends a path and
//! no `dialog:` permission is granted. A cancelled dialog returns `null`.

use std::path::PathBuf;

use lis_core::CommandError;
use lis_engine::{
    ChosenResultFolder, DialogCommand, Engine, Instrument, LoadResultFileInput, LoadedResultFile,
    ResultFolderInput,
};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

#[tauri::command]
pub(crate) async fn load_result_file<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: LoadResultFileInput,
) -> Result<Option<LoadedResultFile>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::LoadResultFile)
        .await?;
    let file = chosen(app, "file", |dialog| {
        dialog
            .add_filter("Result files", &["csv", "txt"])
            .blocking_pick_file()
    })
    .await;
    engine.load_result_file(input, file).await
}

#[tauri::command]
pub(crate) async fn choose_result_folder<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: ResultFolderInput,
) -> Result<Option<ChosenResultFolder>, CommandError> {
    engine
        .preflight_dialog(DialogCommand::ChooseResultFolder)
        .await?;
    let folder = chosen(app, "folder", FileDialogBuilder::blocking_pick_folder).await;
    engine.choose_result_folder(input, folder).await
}

#[tauri::command]
pub(crate) async fn clear_result_folder(
    engine: State<'_, Engine>,
    input: ResultFolderInput,
) -> Result<Instrument, CommandError> {
    engine.clear_result_folder(input).await
}

/// Runs the blocking native dialog `pick` off the main thread; `None` when cancelled.
async fn chosen<R: Runtime>(
    app: AppHandle<R>,
    dialog: &'static str,
    pick: impl FnOnce(FileDialogBuilder<R>) -> Option<FilePath> + Send + 'static,
) -> Option<PathBuf> {
    let picked = tauri::async_runtime::spawn_blocking(move || pick(app.dialog().file())).await;
    match picked {
        Ok(path) => path.and_then(|path| path.into_path().ok()),
        Err(error) => {
            tracing::warn!(%error, dialog, "native dialog stopped");
            None
        }
    }
}
