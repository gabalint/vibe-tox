//! The shell-only `set_window_theme` command: the window's title bar follows the signed-in
//! person's Appearance choice (ticket #4487). Presentation only, so not an engine command:
//! nothing is recorded, and a failure is logged and ignored.

use serde::Deserialize;
use tauri::{Runtime, Theme, WebviewWindow};

/// The theme the UI asks for; absent means "follow the operating system".
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WindowTheme {
    Light,
    Dark,
}

#[tauri::command]
pub(crate) fn set_window_theme<R: Runtime>(window: WebviewWindow<R>, theme: Option<WindowTheme>) {
    let theme = theme.map(|theme| match theme {
        WindowTheme::Light => Theme::Light,
        WindowTheme::Dark => Theme::Dark,
    });
    if let Err(error) = window.set_theme(theme) {
        tracing::warn!(%error, "window theme not changed");
    }
}
