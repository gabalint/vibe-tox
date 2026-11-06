//! The import-attempt list: counters only, filtered and searched in the query.

use super::*;
use crate::model::ImportOutcome;

/// Which attempts the import list shows; the default shows every attempt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportAttemptFilter {
    pub instrument_id: Option<InstrumentId>,
    pub outcome: Option<ImportOutcome>,
    /// Attempts of receipts that began to arrive at or after this instant.
    pub received_from: Option<UtcTimestamp>,
    /// Attempts of receipts that began to arrive before this instant.
    pub received_before: Option<UtcTimestamp>,
    /// Text in a sample label, container label, accession number or file name
    /// the attempt brought; letters match in either case.
    pub search: Option<String>,
}

/// The longest search text accepted.
const SEARCH_MAX_CHARS: usize = 100;

/// Lists import counters only; clinical row text is excluded.
pub fn list_import_attempts(
    conn: &Connection,
    instrument_id: Option<InstrumentId>,
    limit: u32,
    before: Option<UtcTimestamp>,
) -> Result<Vec<ImportAttemptSummary>, ResultError> {
    let filter = ImportAttemptFilter {
        instrument_id,
        ..ImportAttemptFilter::default()
    };
    list_import_attempts_page(conn, &filter, limit, before, None)
}

/// Positions whose sample label, container label or accession number holds the
/// search text (`?9`), for the attempts whose rows or QC observations used them.
const MATCHED_POSITIONS: &str = "SELECT p.position_id FROM ana_v_positions p \
     LEFT JOIN spc_v_containers c ON c.container_id = p.container_id \
     WHERE instr(lower(p.sample_label), ?9) > 0 OR instr(lower(c.label_code), ?9) > 0 \
        OR instr(lower(c.accession_number), ?9) > 0";

/// Cursor page of retained imports. The immutable attempt ID anchors both timestamp
/// and tie-breaker; clock rollback does not make equal-time rows unreachable. The
/// filter and search apply before the page is cut, so every page holds matches only.
pub fn list_import_attempts_page(
    conn: &Connection,
    filter: &ImportAttemptFilter,
    limit: u32,
    before: Option<UtcTimestamp>,
    after_import_attempt_id: Option<ImportAttemptId>,
) -> Result<Vec<ImportAttemptSummary>, ResultError> {
    let search = checked_filter(filter, limit)?;
    let after_time = after_import_attempt_id
        .map(|id| {
            conn.query_row(
                "SELECT processed_at FROM res_import_attempts WHERE id=?1",
                [id],
                |row| row.get::<_, UtcTimestamp>(0),
            )
            .optional()?
            .ok_or(ResultError::NotFound("import cursor"))
        })
        .transpose()?;
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {ATTEMPT_SUMMARY_COLUMNS} FROM res_import_attempts \
         WHERE (?1 IS NULL OR instrument_id = ?1) \
           AND (?2 IS NULL OR processed_at < ?2) \
           AND (?3 IS NULL OR processed_at < ?3 OR (processed_at = ?3 AND id < ?4)) \
           AND (?6 IS NULL OR outcome = ?6) \
           AND ((?7 IS NULL AND ?8 IS NULL) OR receipt_id IN ( \
                SELECT receipt_id FROM ifc_v_receipt_arrivals \
                WHERE (?7 IS NULL OR received_at >= ?7) AND (?8 IS NULL OR received_at < ?8))) \
           AND (?9 IS NULL \
                OR id IN (SELECT import_attempt_id FROM res_measurements \
                          WHERE position_id IN ({MATCHED_POSITIONS}) \
                          UNION SELECT import_attempt_id FROM res_quarantine_items \
                          WHERE instr(lower(sample_label), ?9) > 0 \
                             OR position_id IN ({MATCHED_POSITIONS}) \
                          UNION SELECT import_attempt_id FROM res_set_aside_rows \
                          WHERE instr(lower(sample_label), ?9) > 0) \
                OR receipt_id IN (SELECT receipt_id FROM ana_v_qc_receipt_positions \
                                  WHERE position_id IN ({MATCHED_POSITIONS}) \
                                  UNION SELECT receipt_id FROM ifc_v_receipt_arrivals \
                                  WHERE instr(lower(file_name), ?9) > 0)) \
         ORDER BY processed_at DESC, id DESC LIMIT ?5"
    ))?;
    Ok(statement
        .query_map(
            params![
                filter.instrument_id,
                before,
                after_time,
                after_import_attempt_id,
                limit + 1,
                filter.outcome,
                filter.received_from,
                filter.received_before,
                search,
            ],
            attempt_summary,
        )?
        .collect::<Result<_, _>>()?)
}

/// Refuses a page size, day range or search text the list does not take, and
/// returns the search text trimmed and in lower case (SQLite's `lower` folds
/// ASCII letters only), or `None` when there is none.
fn checked_filter(filter: &ImportAttemptFilter, limit: u32) -> Result<Option<String>, ResultError> {
    if !(1..=500).contains(&limit) {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Request 1 to 500 imports per page.",
        ));
    }
    if let (Some(from), Some(before)) = (filter.received_from, filter.received_before)
        && before <= from
    {
        return Err(crate::error::invalid(
            "receivedBefore",
            "out_of_range",
            "The end of the received period must be after its start.",
        ));
    }
    let search = filter.search.as_deref().map(str::trim).unwrap_or_default();
    if search.chars().count() > SEARCH_MAX_CHARS {
        return Err(crate::error::invalid(
            "search",
            "too_long",
            "Search for at most 100 characters.",
        ));
    }
    Ok((!search.is_empty()).then(|| search.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests;
