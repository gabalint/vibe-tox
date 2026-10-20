//! Push review pagination into its owner; never materialize the full backlog.
use super::{QueueDefinition, WorkQueueCursor, WorkQueueError, WorkQueuePage};
use lis_results::{ReviewQueueCursor, ReviewQueueFilter};

pub(super) fn page(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
    after: Option<&WorkQueueCursor>,
) -> Result<WorkQueuePage, WorkQueueError> {
    let filter = match definition.code {
        "review_ready" => ReviewQueueFilter::Ready,
        "review_stale" => ReviewQueueFilter::Stale,
        "review_blocked" => ReviewQueueFilter::Blocked,
        _ => return Err(WorkQueueError::InvalidReviewCursor),
    };
    let scan = lis_results::scan_review_work_queue(conn, filter, limit, cursor(after)?)?;
    let next = scan.next.map(|cursor| WorkQueueCursor {
        since: cursor.since,
        item_kind: "review_specimen".to_owned(),
        item_id: cursor.specimen_id.to_string(),
    });
    let mut items: Vec<_> = scan
        .items
        .into_iter()
        .map(|row| {
            let title = format!("{} / {}", row.accession_number, row.specimen_type_code);
            super::owner_items::item(
                definition,
                "review_specimen",
                row.specimen_id,
                title,
                row.state,
                row.since,
            )
        })
        .collect();
    for item in &mut items {
        item.has_more = Some(next.is_some());
    }
    Ok(WorkQueuePage {
        items,
        next,
        scanned: scan.scanned,
    })
}

fn cursor(after: Option<&WorkQueueCursor>) -> Result<Option<ReviewQueueCursor>, WorkQueueError> {
    after
        .map(|after| {
            if after.item_kind != "review_specimen" {
                return Err(WorkQueueError::InvalidReviewCursor);
            }
            let specimen_id = after
                .item_id
                .parse()
                .map_err(|_| WorkQueueError::InvalidReviewCursor)?;
            Ok(ReviewQueueCursor {
                since: after.since,
                specimen_id,
            })
        })
        .transpose()
}
