//! Analyzer listener status (public; instrument names and states only).

use lis_core::CommandError;
use lis_engine::{Engine, ListenerStatusView};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_listener_status(
    engine: State<'_, Engine>,
) -> Result<Vec<ListenerStatusView>, CommandError> {
    engine.list_listener_status().await
}
