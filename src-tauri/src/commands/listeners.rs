//! Analyzer listener status (public; instrument names and states only).

use lis_core::CommandError;
use lis_engine::{Engine, ListenerRestartView, ListenerStatusView};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_listener_status(
    engine: State<'_, Engine>,
) -> Result<Vec<ListenerStatusView>, CommandError> {
    engine.list_listener_status().await
}

#[tauri::command]
pub(crate) async fn get_listener_restart(
    engine: State<'_, Engine>,
) -> Result<Option<ListenerRestartView>, CommandError> {
    engine.get_listener_restart().await
}
