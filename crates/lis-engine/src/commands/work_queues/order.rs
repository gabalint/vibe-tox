//! Queue order: oldest first, one task per record, after the cursor, within the limit.

use super::{WorkQueueCursor, WorkQueueItem};

pub(super) fn sort_items(
    items: &mut Vec<WorkQueueItem>,
    limit: u32,
    after: Option<&WorkQueueCursor>,
) {
    items.sort_by(|left, right| {
        (left.since, &left.item_kind, &left.item_id).cmp(&(
            right.since,
            &right.item_kind,
            &right.item_id,
        ))
    });
    // Multiple pending owner events can lead to the same record workspace.
    // Keep one navigation task per record, including its earliest outstanding time.
    let mut seen = std::collections::BTreeSet::new();
    items.retain(|item| seen.insert((item.item_kind.clone(), item.item_id.clone())));
    if let Some(cursor) = after {
        items.retain(|item| {
            (item.since, item.item_kind.as_str(), item.item_id.as_str())
                > (
                    cursor.since,
                    cursor.item_kind.as_str(),
                    cursor.item_id.as_str(),
                )
        });
    }
    let limit = limit as usize;
    let has_more = items.len() > limit;
    items.truncate(limit);
    for item in items {
        item.has_more = Some(has_more);
    }
}
