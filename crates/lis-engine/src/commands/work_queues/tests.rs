use super::*;
use lis_core::{Capability, SessionId, UserId, WorkstationId};

fn principal(capabilities: impl IntoIterator<Item = Capability>) -> Principal {
    Principal::user(
        UserId::generate(),
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities,
    )
}

#[test]
fn queue_visibility_is_owner_capability_filtered() {
    let reviewer = principal([Capability::ReviewResults]);
    assert!(queue_definition("review_ready").is_some());
    assert!(reviewer.require(Capability::ReviewResults).is_ok());
    assert!(reviewer.require(Capability::EnterOrders).is_err());
}

#[test]
fn queue_items_are_oldest_first_with_a_stable_tie_break() -> Result<(), lis_core::ParseError> {
    let old = UtcTimestamp::parse("2026-10-13T10:00:00.000000000Z")?;
    let new = UtcTimestamp::parse("2026-10-13T11:00:00.000000000Z")?;
    let mut items = vec![item("c", new), item("b", old), item("a", old)];
    sort_items(&mut items, 2, None);
    assert_eq!(
        items
            .iter()
            .map(|item| item.item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert!(items.iter().all(|item| item.has_more == Some(true)));
    Ok(())
}

#[test]
fn later_duplicate_does_not_reappear_after_its_earliest_task_cursor()
-> Result<(), lis_core::ParseError> {
    let old = UtcTimestamp::parse("2026-10-13T10:00:00.000000000Z")?;
    let new = UtcTimestamp::parse("2026-10-13T11:00:00.000000000Z")?;
    let cursor = super::WorkQueueCursor {
        since: old,
        item_kind: "synthetic".into(),
        item_id: "a".into(),
    };
    let mut items = vec![item("a", new), item("a", old), item("b", new)];
    sort_items(&mut items, 2, Some(&cursor));
    assert_eq!(
        items
            .iter()
            .map(|item| item.item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
    Ok(())
}

fn item(id: &str, since: UtcTimestamp) -> WorkQueueItem {
    WorkQueueItem {
        queue: "synthetic".to_owned(),
        item_kind: "synthetic".to_owned(),
        item_id: id.to_owned(),
        title: "Synthetic item".to_owned(),
        state: "open".to_owned(),
        state_label: Some("Open".to_owned()),
        since,
        next_action: "Review".to_owned(),
        has_more: Some(false),
    }
}
