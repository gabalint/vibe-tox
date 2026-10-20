//! Thin work-queue adapters (REQ-OPS-004).

use lis_core::CommandError;
use lis_engine::{Engine, ListWorkQueueItemsInput, WorkQueuePage, WorkQueueSummary};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_work_queues(
    engine: State<'_, Engine>,
) -> Result<Vec<WorkQueueSummary>, CommandError> {
    engine.list_work_queues().await
}

#[tauri::command]
pub(crate) async fn list_work_queue_items(
    engine: State<'_, Engine>,
    input: ListWorkQueueItemsInput,
) -> Result<WorkQueuePage, CommandError> {
    engine.list_work_queue_items(input).await
}
