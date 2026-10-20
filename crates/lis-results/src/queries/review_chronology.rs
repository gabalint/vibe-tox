//! Audit-backed legacy ordering and attributable recovery with a new review.
use super::*;

type LatestReview = (TechnicalReviewId, crate::ReviewDecision, ResultRevisionId);

pub(super) fn latest(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<Option<LatestReview>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT r.review_id, r.decision, r.result_revision_id FROM res_v_latest_reviews r \
             JOIN res_result_revisions target ON target.id = ?1 \
               AND target.result_id = r.result_id LIMIT 1",
        )?
        .query_row([revision_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?)
}

pub(super) fn without_latest(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<ReviewStatus, ResultError> {
    let has_history: bool = conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM res_review_items item \
         JOIN res_result_revisions reviewed ON reviewed.id = item.result_revision_id \
         JOIN res_result_revisions target ON target.result_id = reviewed.result_id \
         WHERE target.id = ?1)",
        )?
        .query_row([revision_id], |row| row.get(0))?;
    Ok(if has_history {
        unresolved()
    } else {
        ReviewStatus::None
    })
}

pub(super) fn unique_audit_sequence(
    conn: &Connection,
    review_id: TechnicalReviewId,
) -> Result<Option<i64>, ResultError> {
    let events = lis_audit::succeeded_record_events(
        conn,
        lis_audit::Stream::Clinical,
        "result.submit_review",
        "technical_review",
        &review_id.to_string(),
    )?;
    // Neither a missing event nor multiple succeeded events establishes the
    // selected review's unique causal identity. Never silently take the first.
    Ok(match events.as_slice() {
        [event] => Some(event.seq),
        _ => None,
    })
}

pub(super) fn unresolved() -> ReviewStatus {
    ReviewStatus::Stale {
        reasons: vec![gate(
            "review_chronology_unresolved",
            "Historical review order or audit attribution cannot be confirmed. Inspect the retained evidence and submit a new technical review of the current revisions; do not infer the historical order.",
        )],
    }
}
