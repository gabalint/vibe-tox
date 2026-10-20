//! Application and web engine versions, shown to the user and recorded for support
//! (NFR-33: the web engine version is part of each release's evidence).

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use ts_rs::TS;

/// Versions of the running app and the OS web engine.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub(crate) struct AppInfo {
    app_version: String,
    /// Absent when the OS does not report the web engine version.
    webview_version: Option<String>,
    /// Operating system and CPU architecture, such as `macos-aarch64`.
    platform: String,
}

#[tauri::command]
pub(crate) fn app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    let webview_version = tauri::webview_version()
        .inspect_err(|error| tracing::warn!(%error, "web engine version not reported"))
        .ok();
    AppInfo {
        app_version: app.package_info().version.to_string(),
        webview_version,
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    }
}
