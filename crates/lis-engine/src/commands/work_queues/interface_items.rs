//! Interface queue adapters: unrouted messages and messages not written (REQ-RPT-003).

use super::{QueueDefinition, WorkQueueError, WorkQueueItem};

pub(super) fn interface_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let item = |kind: &str, id: String, title: String, state: &str, since| WorkQueueItem {
        queue: definition.code.to_owned(),
        item_kind: kind.to_owned(),
        item_id: id,
        title,
        state_label: Some(super::catalog::state_label(state)),
        state: state.to_owned(),
        since,
        next_action: definition.next_action.to_owned(),
        has_more: Some(false),
    };
    Ok(match definition.code {
        "unrouted_messages" => lis_interfaces::partner::unrouted_queue(conn, limit)?
            .into_iter()
            .map(|row| {
                item(
                    "unrouted_message",
                    row.message_id.to_string(),
                    row.record,
                    "unrouted",
                    row.since,
                )
            })
            .collect(),
        "interface_failures" => lis_interfaces::partner::failed_queue(conn, limit)?
            .into_iter()
            .map(|row| {
                let title = format!("{} to {}", row.record, row.connection_name);
                item(
                    "failed_delivery",
                    row.delivery_id.to_string(),
                    title,
                    "not_written",
                    row.since,
                )
            })
            .collect(),
        _ => Vec::new(),
    })
}
