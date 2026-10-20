//! Composed review pages preserve the owner's cursor and permission checks.
use crate::m2_lab::{Lab, Role};
use crate::m2_steps::JOURNEY;
use lis_core::ErrorCode;
use lis_engine::{ListWorkQueueItemsInput, WorkQueueCursor, WorkQueueItem};

fn input(after: Option<WorkQueueCursor>) -> ListWorkQueueItemsInput {
    ListWorkQueueItemsInput {
        queue: "review_ready".into(),
        limit: 2,
        after,
    }
}

fn cursor(row: &WorkQueueItem) -> WorkQueueCursor {
    WorkQueueCursor {
        since: row.since,
        item_kind: row.item_kind.clone(),
        item_id: row.item_id.clone(),
    }
}

#[tokio::test]
async fn review_pages_continue_at_equal_times_and_reject_invalid_cursors() {
    let lab = Lab::start(lis_analytical::RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.sign_in(Role::Reviewer).await;
    let first = lab
        .engine()
        .list_work_queue_items(input(None))
        .await
        .unwrap();
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.scanned, 2);
    assert!(first.items.iter().all(|row| row.has_more == Some(true)));
    let after = first.next.clone().unwrap();
    assert_eq!(after, cursor(&first.items[1]));
    let second = lab
        .engine()
        .list_work_queue_items(input(Some(after.clone())))
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.scanned, 1);
    assert!(second.next.is_none());
    assert_eq!(second.items[0].has_more, Some(false));
    let actual = first
        .items
        .iter()
        .chain(&second.items)
        .map(|row| row.item_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = lab
        .samples
        .iter()
        .take(3)
        .map(|sample| sample.specimen_id.to_string())
        .collect();
    assert_eq!(actual, expected);
    assert!(
        first
            .items
            .iter()
            .chain(&second.items)
            .all(|row| row.since == after.since)
    );
    let empty = lab
        .engine()
        .list_work_queue_items(input(Some(cursor(&second.items[0]))))
        .await
        .unwrap();
    assert!(empty.items.is_empty());
    assert!(empty.next.is_none());
    assert_eq!(empty.scanned, 0);
    assert_composed_audits(&lab);
    assert_invalid_cursors(&lab, after).await;
    lab.sign_in(Role::Accessioner).await;
    assert_eq!(
        lab.engine()
            .list_work_queue_items(input(None))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
}

fn assert_composed_audits(lab: &Lab) {
    let conn = lab.t().conn();
    let mut statement = conn
        .prepare(
            "SELECT user.username, event.authority, json_extract(event.details,'$.queue'), \
         json_extract(event.details,'$.returned'), json_extract(event.details,'$.scanned') \
         FROM audit_events event JOIN auth_users user ON user.id = event.principal_id \
         WHERE event.stream = 'access' AND event.action = 'operations.list_work_queue_items' \
         AND event.outcome = 'succeeded' ORDER BY event.seq",
        )
        .unwrap();
    let rows: Vec<(String, String, String, i64, i64)> = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.len(), 3);
    for ((actor, authority, queue, returned, scanned), expected) in rows.into_iter().zip([2, 1, 0])
    {
        assert_eq!(
            (actor.as_str(), authority.as_str(), queue.as_str()),
            ("rev1", "review_results", "review_ready")
        );
        assert_eq!((returned, scanned), (expected, expected));
    }
}

async fn assert_invalid_cursors(lab: &Lab, after: WorkQueueCursor) {
    for invalid in [
        WorkQueueCursor {
            item_kind: "order".into(),
            ..after.clone()
        },
        WorkQueueCursor {
            item_id: "not-a-specimen-id".into(),
            ..after
        },
    ] {
        assert_eq!(
            lab.engine()
                .list_work_queue_items(input(Some(invalid)))
                .await
                .unwrap_err()
                .code(),
            ErrorCode::Validation
        );
    }
}

#[tokio::test]
async fn detailed_scan_reports_progress_and_attributes_reads_to_the_reviewer() {
    let lab = Lab::start(lis_analytical::RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.sign_in(Role::Reviewer).await;
    let mut request = lis_engine::ListReviewQueueInput {
        filter: lis_results::ReviewQueueFilter::Ready,
        limit: 2,
        after: None,
    };
    let first = lab.engine().list_review_queue(request).await.unwrap();
    assert_eq!((first.items.len(), first.scanned), (2, 2));
    request = lis_engine::ListReviewQueueInput {
        filter: lis_results::ReviewQueueFilter::Ready,
        limit: 2,
        after: first.next,
    };
    assert!(request.after.is_some());
    let last = lab.engine().list_review_queue(request).await.unwrap();
    assert_eq!((last.items.len(), last.scanned), (1, 1));
    assert!(last.next.is_none());
    assert!(
        first
            .items
            .iter()
            .all(|item| item.specimen_id != last.items[0].specimen_id)
    );
    let request = || lis_engine::ListReviewQueueInput {
        filter: lis_results::ReviewQueueFilter::Blocked,
        limit: 2,
        after: None,
    };
    let empty = lab.engine().list_review_queue(request()).await.unwrap();
    assert!(empty.items.is_empty());
    assert!(empty.next.is_none());
    assert_eq!(
        empty.scanned, 3,
        "empty means all three candidates were checked"
    );
    assert_scan_audit(&lab);
    let summaries = lab.engine().list_work_queues().await.unwrap();
    for code in ["review_stale", "review_blocked"] {
        let summary = summaries
            .iter()
            .find(|summary| summary.queue == code)
            .unwrap();
        assert_eq!(
            summary.count, None,
            "the overview must not claim an unchecked zero"
        );
        assert!(!summary.count_capped);
    }
    lab.sign_in(Role::Accessioner).await;
    assert_eq!(
        lab.engine()
            .list_review_queue(request())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
}

fn assert_scan_audit(lab: &Lab) {
    let (actor, authority, returned, scanned): (String, String, i64, i64) = lab
        .t()
        .conn()
        .query_row(
            "SELECT user.username, event.authority, json_extract(event.details,'$.returned'), \
             json_extract(event.details,'$.scanned') FROM audit_events event \
             JOIN auth_users user ON user.id = event.principal_id \
             WHERE event.action = 'result.list_review_queue' AND event.outcome = 'succeeded' \
             ORDER BY event.seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (actor.as_str(), authority.as_str(), returned, scanned),
        ("rev1", "review_results", 0, 3)
    );
}
