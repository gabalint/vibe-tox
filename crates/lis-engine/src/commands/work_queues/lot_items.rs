//! Active lots that expire within the laboratory's lot expiry warning period (#4557). The
//! analytical module owns the selection; the period is an operations setting.

use lis_core::UtcTimestamp;

use super::{QueueDefinition, WorkQueueError, WorkQueueItem};

pub(super) fn expiring_lot_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    now: UtcTimestamp,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let warn_days = lis_ops::settings::current(conn)?.lot_expiry_warn_days;
    Ok(lis_analytical::lots_expiring_soon(conn, now, warn_days)?
        .into_iter()
        .map(|item| {
            let days = item.lot.days_left.unwrap_or_default();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "lot".to_owned(),
                item_id: item.lot.lot_id.to_string(),
                title: item.lot.lot_number,
                state: "expiring_soon".to_owned(),
                state_label: Some(expires_in(days)),
                since: item.since,
                next_action: definition.next_action.to_owned(),
                has_more: Some(false),
            }
        })
        .collect())
}

/// "Expires today", "Expires in 1 day" or "Expires in N days".
pub(super) fn expires_in(days: u32) -> String {
    match days {
        0 => "Expires today".to_owned(),
        1 => "Expires in 1 day".to_owned(),
        _ => format!("Expires in {days} days"),
    }
}
