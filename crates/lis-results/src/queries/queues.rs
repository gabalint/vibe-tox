//! Technical-review work queues without patient details for operational composition
//! (REQ-OPS-004, REQ-OPS-013). The import exception queues are in
//! `exception_queues`, the results-expected queue in `awaiting`.
//!
//! Review queues call the existing technical-review status and gate queries,
//! so staleness and blocking rules remain in their existing single homes.

use lis_core::UtcTimestamp;
use lis_specimens::SpecimenId;
use rusqlite::{Connection, params};

use super::read_context::ReviewRead;
use super::review::{review_gates_read, review_status_read};
use super::specimen::{review_queue_labels, review_queue_metadata};
use crate::views::{ReviewQueueEntry, ReviewStatus};
use crate::{ResultError, ResultRevisionId, ReviewQueueFilter};

mod candidates;
mod scan;
use candidates::{ReviewCandidate, candidate_row, candidate_statement};
pub use scan::{ReviewQueuePage, ReviewWorkQueuePage, scan_review_queue, scan_review_work_queue};

#[derive(Debug)]
struct ReviewQueueRow {
    candidate: ReviewCandidate,
    facts: ReviewFacts,
}

#[derive(Debug, Default)]
struct ReviewFacts {
    unreviewed: i64,
    stale: i64,
    blocked: i64,
    first_gate: Option<String>,
}

/// One specimen awaiting technical-review work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultReviewQueueItem {
    /// The specimen to open.
    pub specimen_id: SpecimenId,
    /// Operational title used by the work-queue screen.
    pub accession_number: String,
    /// The specimen matrix code.
    pub specimen_type_code: String,
    /// `ready`, `stale`, or the first blocking gate code.
    pub state: String,
    /// When the specimen first entered this queue state.
    pub since: UtcTimestamp,
}

/// Current result lines ready for their first technical review.
pub fn review_ready_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<ResultReviewQueueItem>, ResultError> {
    review_items(conn, ReviewQueueFilter::Ready, limit)
}

/// Technical reviews made stale by a changed result or dependency.
pub fn review_stale_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<ResultReviewQueueItem>, ResultError> {
    review_items(conn, ReviewQueueFilter::Stale, limit)
}

/// Specimens whose current result lines have at least one review gate.
pub fn review_blocked_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<ResultReviewQueueItem>, ResultError> {
    review_items(conn, ReviewQueueFilter::Blocked, limit)
}

fn review_items(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
) -> Result<Vec<ResultReviewQueueItem>, ResultError> {
    work_items(conn, filter, bounded(limit), None)
}

/// One bounded operational page, including one extra item to signal continuation.
/// Uses the same membership, ordering and safety rules as the detailed review page.
pub fn review_work_queue_page(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewQueueCursor>,
) -> Result<Vec<ResultReviewQueueItem>, ResultError> {
    validate_limit(limit)?;
    work_items(conn, filter, limit + 1, after.map(Into::into))
}

fn work_items(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewCandidate>,
) -> Result<Vec<ResultReviewQueueItem>, ResultError> {
    review_queue_rows(conn, filter, limit, after)?
        .into_iter()
        .map(|row| work_item(conn, filter, row))
        .collect()
}

fn work_item(
    conn: &Connection,
    filter: ReviewQueueFilter,
    row: ReviewQueueRow,
) -> Result<ResultReviewQueueItem, ResultError> {
    let (accession_number, specimen_type_code, _) =
        review_queue_labels(conn, row.candidate.specimen_id)?;
    Ok(ResultReviewQueueItem {
        specimen_id: row.candidate.specimen_id,
        accession_number,
        specimen_type_code,
        state: queue_state(filter, &row.facts),
        since: row.candidate.since,
    })
}

pub(super) fn review_queue_entries(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
) -> Result<Vec<ReviewQueueEntry>, ResultError> {
    review_queue_page(conn, filter, limit, None)
}

/// A stable review page cursor, preserving equal timestamps.
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, ts_rs::TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewQueueCursor {
    /// Queue ordering timestamp from the last displayed or explicitly checked candidate.
    pub since: UtcTimestamp,
    /// Its stable specimen identity.
    pub specimen_id: SpecimenId,
}

/// Bounded review page. Refresh restarts paging when concurrent state changes move rows.
pub fn review_queue_page(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewQueueCursor>,
) -> Result<Vec<ReviewQueueEntry>, ResultError> {
    validate_limit(limit)?;
    review_queue_rows(conn, filter, limit + 1, after.map(Into::into))?
        .into_iter()
        .map(|row| detailed_item(conn, row))
        .collect()
}

fn detailed_item(conn: &Connection, row: ReviewQueueRow) -> Result<ReviewQueueEntry, ResultError> {
    let metadata = review_queue_metadata(conn, row.candidate.specimen_id)?;
    Ok(ReviewQueueEntry {
        subject: metadata.subject,
        since: row.candidate.since,
        specimen_id: row.candidate.specimen_id,
        accession_number: metadata.accession_number,
        specimen_type_code: metadata.specimen_type_code,
        lines: metadata.lines,
        pending: metadata.pending,
        stale: row.facts.stale,
        blocked: row.facts.blocked,
    })
}

fn validate_limit(limit: u32) -> Result<(), ResultError> {
    if !(1..=500).contains(&limit) {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Request 1 to 500 specimens per page.",
        ));
    }
    Ok(())
}

impl From<ReviewQueueCursor> for ReviewCandidate {
    fn from(cursor: ReviewQueueCursor) -> Self {
        Self {
            since: cursor.since,
            specimen_id: cursor.specimen_id,
        }
    }
}

/// Streams candidates oldest first from one grouped query. Sparse dynamic matches
/// still inspect historical candidates, without regrouping all history per page.
fn review_queue_rows(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
    after: Option<ReviewCandidate>,
) -> Result<Vec<ReviewQueueRow>, ResultError> {
    let _read_snapshot = if conn.is_autocommit() {
        Some(conn.unchecked_transaction()?)
    } else {
        None
    };
    let mut read = ReviewRead::new(conn);
    let limit = limit.max(1) as usize;
    let mut queue = Vec::new();
    let mut statement = candidate_statement(conn, filter)?;
    let candidates = statement.query_map(
        params![after.map(|c| c.since), after.map(|c| c.specimen_id)],
        candidate_row,
    )?;
    for candidate in candidates {
        queue.extend(queue_row(&mut read, filter, candidate?)?);
        if queue.len() == limit {
            break;
        }
    }
    Ok(queue)
}

fn queue_row(
    read: &mut ReviewRead<'_>,
    filter: ReviewQueueFilter,
    candidate: ReviewCandidate,
) -> Result<Option<ReviewQueueRow>, ResultError> {
    let conn = read.conn;
    read.begin_specimen();
    let facts = review_facts(read, candidate.specimen_id, filter)?;
    let quarantine = has_open_quarantine(conn, candidate.specimen_id)?;
    let include = match filter {
        ReviewQueueFilter::Ready => {
            facts.unreviewed > 0 && facts.blocked == 0 && facts.stale == 0 && !quarantine
        }
        ReviewQueueFilter::Stale => facts.stale > 0,
        ReviewQueueFilter::Blocked => facts.blocked > 0,
        ReviewQueueFilter::Quarantine => quarantine,
    };
    if !include {
        return Ok(None);
    }
    Ok(Some(ReviewQueueRow { candidate, facts }))
}

fn review_facts(
    read: &mut ReviewRead<'_>,
    specimen_id: SpecimenId,
    filter: ReviewQueueFilter,
) -> Result<ReviewFacts, ResultError> {
    let revisions = current_revision_ids(read.conn, specimen_id)?;
    let mut facts = ReviewFacts::default();
    // Evaluate the requested predicate first. A nonmatch does not need unrelated
    // counts; every returned row still has both complete counts and its first gate.
    if filter == ReviewQueueFilter::Blocked {
        append_gates(read, &revisions, &mut facts)?;
        if facts.blocked > 0 {
            append_statuses(read, &revisions, &mut facts)?;
        }
    } else {
        append_statuses(read, &revisions, &mut facts)?;
        if filter != ReviewQueueFilter::Stale || facts.stale > 0 {
            append_gates(read, &revisions, &mut facts)?;
        }
    }
    Ok(facts)
}

fn append_statuses(
    read: &mut ReviewRead<'_>,
    revisions: &[ResultRevisionId],
    facts: &mut ReviewFacts,
) -> Result<(), ResultError> {
    for revision in revisions {
        match review_status_read(read, *revision)? {
            ReviewStatus::None => facts.unreviewed += 1,
            ReviewStatus::Stale { .. } => facts.stale += 1,
            ReviewStatus::Current { .. } => {}
        }
    }
    Ok(())
}

fn append_gates(
    read: &mut ReviewRead<'_>,
    revisions: &[ResultRevisionId],
    facts: &mut ReviewFacts,
) -> Result<(), ResultError> {
    for revision in revisions {
        let gates = review_gates_read(read, *revision)?;
        if !gates.is_empty() {
            facts.blocked += 1;
            if facts.first_gate.is_none() {
                facts.first_gate = gates.first().map(|gate| gate.code.clone());
            }
        }
    }
    Ok(())
}

fn current_revision_ids(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<ResultRevisionId>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT result.current_revision_id FROM res_results result \
             JOIN res_result_revisions current ON current.id = result.current_revision_id \
             WHERE current.specimen_id = ?1 ORDER BY current.created_at, current.id",
        )?
        .query_map([specimen_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

fn has_open_quarantine(conn: &Connection, specimen_id: SpecimenId) -> Result<bool, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM res_quarantine_items quarantine \
         JOIN ana_v_positions position ON position.position_id = quarantine.position_id \
         JOIN spc_v_containers container ON container.container_id = position.container_id \
         WHERE container.specimen_id = ?1 AND quarantine.state = 'open')",
        )?
        .query_row([specimen_id], |row| row.get(0))?)
}

fn queue_state(filter: ReviewQueueFilter, facts: &ReviewFacts) -> String {
    match filter {
        ReviewQueueFilter::Ready => "ready".to_owned(),
        ReviewQueueFilter::Stale => "stale".to_owned(),
        ReviewQueueFilter::Quarantine => "open".to_owned(),
        ReviewQueueFilter::Blocked => facts
            .first_gate
            .clone()
            .unwrap_or_else(|| "blocked".to_owned()),
    }
}

pub(super) fn bounded(limit: u32) -> u32 {
    limit.max(1)
}
#[cfg(test)]
mod tests;
