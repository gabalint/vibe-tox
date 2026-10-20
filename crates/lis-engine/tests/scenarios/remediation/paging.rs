//! More than the former 500-item source cap remains reachable at equal timestamps.
use crate::m2_lab::{Lab, Role};
use lis_engine::{ListWorkQueueItemsInput, WorkQueueCursor};
use std::collections::BTreeSet;

#[tokio::test]
async fn all_503_active_deviations_are_reachable_without_closing_earlier_work() {
    let lab = Lab::start(lis_analytical::RevisionIndicator::None).await;
    lab.sign_in(Role::Quality).await;
    let mut expected = BTreeSet::new();
    for index in 0..503 {
        let deviation = lab
            .engine()
            .open_deviation(lis_analytical::OpenDeviationInput {
                scope: lis_analytical::DeviationTarget::Run(lab.run),
                analyte_id: None,
                description: format!("Synthetic equal-time incident {index}"),
                reason: "Synthetic queue boundary verification".into(),
            })
            .await
            .unwrap();
        expected.insert(deviation.deviation_id.to_string());
    }
    let summaries = lab.engine().list_work_queues().await.unwrap();
    let summary = summaries
        .iter()
        .find(|q| q.queue == "open_deviations")
        .unwrap();
    assert!(summary.count_capped);
    let mut seen = BTreeSet::new();
    let mut after = None;
    let mut sizes = Vec::new();
    loop {
        let rows = lab
            .engine()
            .list_work_queue_items(ListWorkQueueItemsInput {
                queue: "open_deviations".into(),
                limit: 200,
                after: after.clone(),
            })
            .await
            .unwrap()
            .items;
        sizes.push(rows.len());
        for row in &rows {
            assert!(seen.insert(row.item_id.clone()), "cursor repeated a row");
        }
        let last = rows.last().expect("hasMore must lead to a nonempty page");
        if !rows[0].has_more.unwrap_or(false) {
            break;
        }
        after = Some(WorkQueueCursor {
            since: last.since,
            item_kind: last.item_kind.clone(),
            item_id: last.item_id.clone(),
        });
    }
    assert_eq!(sizes, [200, 200, 103]);
    assert_eq!(seen, expected);
    let closed: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM ana_deviations WHERE state='closed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(closed, 0);
}
