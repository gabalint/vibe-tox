//! Bounded clinical candidate checks with an explicit continuation even when no item matches.
use super::*;

/// At most this many candidate specimens undergo safety evaluation in one request.
const SCAN_LIMIT: usize = 500;

/// A detailed review page. Empty items do not prove an empty queue while `next` exists.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewQueuePage {
    pub items: Vec<ReviewQueueEntry>,
    pub next: Option<ReviewQueueCursor>,
    #[ts(type = "number")]
    pub scanned: usize,
}

/// Operational counterpart without patient details using the same candidate scan and gates.
#[derive(Debug, Clone)]
pub struct ReviewWorkQueuePage {
    pub items: Vec<ResultReviewQueueItem>,
    pub next: Option<ReviewQueueCursor>,
    pub scanned: usize,
}

/// Evaluates a bounded set of candidates, preserving all historical work through continuation.
pub fn scan_review_queue(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewQueueCursor>,
) -> Result<ReviewQueuePage, ResultError> {
    let scan = scan_rows(conn, filter, limit, after)?;
    Ok(ReviewQueuePage {
        items: scan
            .items
            .into_iter()
            .map(|row| detailed_item(conn, row))
            .collect::<Result<_, _>>()?,
        next: scan.next,
        scanned: scan.scanned,
    })
}

/// Operational page without patient details. The continuation names the last checked candidate.
pub fn scan_review_work_queue(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewQueueCursor>,
) -> Result<ReviewWorkQueuePage, ResultError> {
    let scan = scan_rows(conn, filter, limit, after)?;
    Ok(ReviewWorkQueuePage {
        items: scan
            .items
            .into_iter()
            .map(|row| work_item(conn, filter, row))
            .collect::<Result<_, _>>()?,
        next: scan.next,
        scanned: scan.scanned,
    })
}

struct ScanRows {
    items: Vec<ReviewQueueRow>,
    next: Option<ReviewQueueCursor>,
    scanned: usize,
}

fn scan_rows(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewQueueCursor>,
) -> Result<ScanRows, ResultError> {
    validate_limit(limit)?;
    let _snapshot = if conn.is_autocommit() {
        Some(conn.unchecked_transaction()?)
    } else {
        None
    };
    let mut read = ReviewRead::new(conn);
    let mut result = ScanRows {
        items: Vec::new(),
        next: None,
        scanned: 0,
    };
    let mut last = None;
    let mut statement = candidate_statement(conn, filter)?;
    let candidates = statement.query_map(
        params![after.map(|c| c.since), after.map(|c| c.specimen_id)],
        candidate_row,
    )?;
    for candidate in candidates {
        let candidate = candidate?;
        if result.scanned == SCAN_LIMIT || result.items.len() == limit as usize {
            result.next = last;
            break;
        }
        result.scanned += 1;
        last = Some(ReviewQueueCursor {
            since: candidate.since,
            specimen_id: candidate.specimen_id,
        });
        result
            .items
            .extend(queue_row(&mut read, filter, candidate)?);
    }
    Ok(result)
}
