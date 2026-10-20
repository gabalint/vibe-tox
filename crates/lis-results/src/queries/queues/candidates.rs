//! Stable candidate enumeration; dynamic membership stays in the existing gates.
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct ReviewCandidate {
    pub(super) specimen_id: SpecimenId,
    pub(super) since: UtcTimestamp,
}

pub(super) fn candidate_statement(
    conn: &Connection,
    filter: ReviewQueueFilter,
) -> rusqlite::Result<rusqlite::CachedStatement<'_>> {
    let grouped = match filter {
        ReviewQueueFilter::Ready => {
            "SELECT specimen_id, min(since) AS since FROM res_unreviewed_results \
             GROUP BY specimen_id"
        }
        ReviewQueueFilter::Stale => {
            "SELECT current.specimen_id AS specimen_id, min(review.reviewed_at) AS since \
             FROM res_results result \
             JOIN res_result_revisions current ON current.id = result.current_revision_id \
             JOIN res_result_revisions reviewed ON reviewed.result_id = result.id \
             JOIN res_review_items item ON item.result_revision_id = reviewed.id \
             JOIN res_technical_reviews review ON review.id = item.review_id \
             GROUP BY current.specimen_id"
        }
        ReviewQueueFilter::Blocked => {
            "SELECT current.specimen_id AS specimen_id, min(current.created_at) AS since \
             FROM res_results result \
             JOIN res_result_revisions current ON current.id = result.current_revision_id \
             GROUP BY current.specimen_id"
        }
        ReviewQueueFilter::Quarantine => {
            "SELECT container.specimen_id AS specimen_id, min(attempt.processed_at) AS since \
             FROM res_quarantine_items quarantine \
             JOIN res_import_attempts attempt ON attempt.id = quarantine.import_attempt_id \
             JOIN ana_v_positions position ON position.position_id = quarantine.position_id \
             JOIN spc_v_containers container ON container.container_id = position.container_id \
             WHERE quarantine.state = 'open' GROUP BY container.specimen_id"
        }
    };
    let sql = format!(
        "SELECT specimen_id, since FROM ({grouped}) \
         WHERE ?1 IS NULL OR since > ?1 OR (since = ?1 AND specimen_id > ?2) \
         ORDER BY since, specimen_id "
    );
    conn.prepare_cached(&sql)
}

pub(super) fn candidate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewCandidate> {
    Ok(ReviewCandidate {
        specimen_id: row.get(0)?,
        since: row.get(1)?,
    })
}
