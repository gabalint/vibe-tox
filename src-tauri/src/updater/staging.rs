//! The one staged update package: written to the app's cache folder after its signature is
//! verified, verified again before reuse, and looked up for approval and installation.

use std::path::PathBuf;

use lis_core::CommandError;
use tauri::{AppHandle, Manager, Runtime};

use super::errors::{internal, not_staged};
use super::{DownloadedUpdateView, Updates};

/// Writes the verified package to the app's cache folder in place of any earlier one.
pub(super) async fn stage<R: Runtime>(
    app: &AppHandle<R>,
    file_name: &str,
    bytes: Vec<u8>,
) -> Result<PathBuf, CommandError> {
    let folder = app
        .path()
        .app_cache_dir()
        .map_err(|_| internal("cache folder not resolved"))?
        .join("updates");
    let path = folder.join(file_name);
    let written = tokio::task::spawn_blocking({
        let path = path.clone();
        move || -> std::io::Result<()> {
            // One staged package at a time.
            if folder.exists() {
                std::fs::remove_dir_all(&folder)?;
            }
            std::fs::create_dir_all(&folder)?;
            std::fs::write(&path, bytes)
        }
    })
    .await
    .map_err(|_| internal("staging task stopped"))?;
    written.map_err(|error| {
        tracing::warn!(error = ?error.kind(), "update package not staged");
        internal("update package not staged")
    })?;
    Ok(path)
}

/// The staged package of `version`, if its file still matches its signature.
pub(super) async fn still_staged(updates: &Updates, version: &str) -> Option<DownloadedUpdateView> {
    let (view, update, path) = {
        let staged = updates.staged();
        let staged = staged
            .as_ref()
            .filter(|staged| staged.view.version == version)?;
        (
            staged.view.clone(),
            staged.update.clone(),
            staged.path.clone(),
        )
    };
    let verified = tokio::task::spawn_blocking(move || {
        std::fs::read(&path)
            .ok()
            .is_some_and(|bytes| update.verify(&bytes).is_ok())
    })
    .await
    .unwrap_or(false);
    if verified {
        return Some(view);
    }
    tracing::warn!(%version, "staged update package no longer verifies; downloading again");
    *updates.staged() = None;
    None
}

/// The staged package's path when it is `version`.
pub(crate) fn staged_path(updates: &Updates, version: &str) -> Result<PathBuf, CommandError> {
    updates
        .staged()
        .as_ref()
        .filter(|staged| staged.view.version == version)
        .map(|staged| staged.path.clone())
        .ok_or_else(not_staged)
}
