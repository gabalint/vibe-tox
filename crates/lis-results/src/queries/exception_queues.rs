//! PHI-free import exception queues for operational composition (REQ-OPS-004):
//! open quarantine rows and performed-not-ordered detections, each row in
//! exactly one queue.

use lis_core::UtcTimestamp;
use lis_specimens::SpecimenId;
use rusqlite::Connection;

use super::queues::bounded;
use crate::{ImportAttemptId, QuarantineItemId, ResultError};

/// One actionable imported row. Raw source values and sample labels are
/// intentionally excluded from this operational contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultExceptionQueueItem {
    /// The quarantined source row.
    pub quarantine_item_id: QuarantineItemId,
    /// The import attempt whose detail screen owns the row.
    pub import_attempt_id: ImportAttemptId,
    /// The linked specimen, when the row could be placed on a run position.
    pub specimen_id: Option<SpecimenId>,
    /// The linked PHI-free accession number, when known.
    pub accession_number: Option<String>,
    /// The quarantine reason code.
    pub state: String,
    /// When the import attempt processed the row.
    pub since: UtcTimestamp,
}

/// Open quarantine rows, oldest import first. Performed-not-ordered rows
/// belong only to [`out_of_scope_queue`], so each row has one owner queue.
pub fn open_quarantine_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<ResultExceptionQueueItem>, ResultError> {
    exception_queue(
        conn,
        "SELECT quarantine.id, quarantine.import_attempt_id, container.specimen_id, \
                container.accession_number, quarantine.reason, attempt.processed_at \
         FROM res_quarantine_items quarantine \
         JOIN res_import_attempts attempt ON attempt.id = quarantine.import_attempt_id \
         LEFT JOIN ana_v_positions position ON position.position_id = quarantine.position_id \
         LEFT JOIN spc_v_containers container ON container.container_id = position.container_id \
         WHERE quarantine.state = 'open' AND quarantine.reason <> 'not_in_ordered_scope' \
         ORDER BY attempt.processed_at, quarantine.id LIMIT ?1",
        limit,
    )
}

/// Open performed-not-ordered detections across every specimen.
pub fn out_of_scope_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<ResultExceptionQueueItem>, ResultError> {
    exception_queue(
        conn,
        "SELECT quarantine.id, quarantine.import_attempt_id, container.specimen_id, \
                container.accession_number, quarantine.reason, attempt.processed_at \
         FROM res_quarantine_items quarantine \
         JOIN res_import_attempts attempt ON attempt.id = quarantine.import_attempt_id \
         LEFT JOIN ana_v_positions position ON position.position_id = quarantine.position_id \
         LEFT JOIN spc_v_containers container ON container.container_id = position.container_id \
         WHERE quarantine.state = 'open' AND quarantine.reason = 'not_in_ordered_scope' \
         ORDER BY attempt.processed_at, quarantine.id LIMIT ?1",
        limit,
    )
}

fn exception_queue(
    conn: &Connection,
    sql: &str,
    limit: u32,
) -> Result<Vec<ResultExceptionQueueItem>, ResultError> {
    Ok(conn
        .prepare_cached(sql)?
        .query_map([bounded(limit)], |row| {
            Ok(ResultExceptionQueueItem {
                quarantine_item_id: row.get(0)?,
                import_attempt_id: row.get(1)?,
                specimen_id: row.get(2)?,
                accession_number: row.get(3)?,
                state: row.get(4)?,
                since: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn exception_queues_are_exclusive_bounded_oldest_first_and_phi_free()
    -> Result<(), Box<dyn std::error::Error>> {
        let conn = exception_connection()?;
        let newer = insert_exception(
            &conn,
            "2026-10-13T10:00:00.000000000Z",
            "unknown_analyte",
            "open",
        )?;
        let older = insert_exception(
            &conn,
            "2026-10-13T09:30:00.000000000Z",
            "malformed_value",
            "open",
        )?;
        let not_ordered = insert_exception(
            &conn,
            "2026-10-13T09:00:00.000000000Z",
            "not_in_ordered_scope",
            "open",
        )?;
        insert_exception(
            &conn,
            "2026-10-13T08:00:00.000000000Z",
            "not_in_ordered_scope",
            "rejected",
        )?;

        let first = open_quarantine_queue(&conn, 1)?;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].quarantine_item_id, older);
        // The performed-not-ordered row has its own queue only.
        let general: Vec<_> = open_quarantine_queue(&conn, 10)?
            .into_iter()
            .map(|item| item.quarantine_item_id)
            .collect();
        assert_eq!(general, [older, newer]);
        let out_of_scope = out_of_scope_queue(&conn, 10)?;
        assert_eq!(out_of_scope.len(), 1);
        assert_eq!(out_of_scope[0].quarantine_item_id, not_ordered);
        assert_eq!(out_of_scope[0].state, "not_in_ordered_scope");
        assert_eq!(out_of_scope[0].accession_number, None);
        Ok(())
    }

    fn exception_connection() -> rusqlite::Result<Connection> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE res_import_attempts (id TEXT PRIMARY KEY, processed_at TEXT) STRICT;
             CREATE TABLE res_quarantine_items (
                id TEXT PRIMARY KEY, import_attempt_id TEXT, position_id TEXT,
                reason TEXT, state TEXT
             ) STRICT;
             CREATE TABLE ana_v_positions (position_id TEXT, container_id TEXT) STRICT;
             CREATE TABLE spc_v_containers (
                container_id TEXT, specimen_id TEXT, accession_number TEXT
             ) STRICT;",
        )?;
        Ok(conn)
    }

    fn insert_exception(
        conn: &Connection,
        processed_at: &str,
        reason: &str,
        state: &str,
    ) -> Result<QuarantineItemId, Box<dyn std::error::Error>> {
        let attempt = ImportAttemptId::generate();
        let item = QuarantineItemId::generate();
        conn.execute(
            "INSERT INTO res_import_attempts VALUES (?1, ?2)",
            params![attempt, processed_at],
        )?;
        conn.execute(
            "INSERT INTO res_quarantine_items VALUES (?1, ?2, NULL, ?3, ?4)",
            params![item, attempt, reason, state],
        )?;
        Ok(item)
    }
}
