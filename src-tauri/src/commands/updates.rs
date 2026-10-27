//! Thin in-app update adapters (ADR 0020; REQ-OPS-006, NFR-24): the engine authorizes each
//! request before the shell contacts the release repository ([`crate::updater`]), records the
//! approval of the downloaded package, and allows its installation. The UI never sends an
//! address or a path.

use lis_core::CommandError;
use lis_engine::{Engine, RecordUpdateApprovalInput, UpdateApprovalView, UpdateRequest};
use serde::Deserialize;
use tauri::{AppHandle, Runtime, State};
use ts_rs::TS;

use crate::updater::{self, DownloadUpdateInput, DownloadedUpdateView, UpdateFeedView, Updates};

/// Input of `install_update`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub(crate) struct InstallUpdateInput {
    /// The staged, approved version.
    version: String,
}

#[tauri::command]
pub(crate) async fn check_release_update<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    updates: State<'_, Updates>,
) -> Result<UpdateFeedView, CommandError> {
    engine.preflight_update(UpdateRequest::CheckRelease).await?;
    updater::release_feed(&app, &updates).await
}

#[tauri::command]
pub(crate) async fn list_nightly_builds<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    updates: State<'_, Updates>,
) -> Result<UpdateFeedView, CommandError> {
    engine.preflight_update(UpdateRequest::ListNightly).await?;
    updater::nightly_feed(&app, &updates).await
}

#[tauri::command]
pub(crate) async fn download_update<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    updates: State<'_, Updates>,
    input: DownloadUpdateInput,
) -> Result<DownloadedUpdateView, CommandError> {
    engine.preflight_update(UpdateRequest::Download).await?;
    updater::download(&app, &updates, input).await
}

/// Records the approval of the staged package of `input.version`.
#[tauri::command]
pub(crate) async fn approve_downloaded_update(
    engine: State<'_, Engine>,
    updates: State<'_, Updates>,
    input: RecordUpdateApprovalInput,
) -> Result<UpdateApprovalView, CommandError> {
    engine.preflight_update(UpdateRequest::Approve).await?;
    let package = updater::staged_path(&updates, &input.version)?;
    engine.approve_downloaded_update(input, package).await
}

/// Installs the staged, approved package and restarts into it.
#[tauri::command]
pub(crate) async fn install_update<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    updates: State<'_, Updates>,
    input: InstallUpdateInput,
) -> Result<(), CommandError> {
    engine.preflight_update(UpdateRequest::Install).await?;
    updater::install(&app, &engine, &updates, &input.version).await
}
