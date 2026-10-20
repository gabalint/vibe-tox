//! Import-attempt list and audited-detail read models.

use super::*;

/// Lists import counters only; clinical row text is excluded.
pub fn list_import_attempts(
    conn: &Connection,
    instrument_id: Option<InstrumentId>,
    limit: u32,
    before: Option<UtcTimestamp>,
) -> Result<Vec<ImportAttemptSummary>, ResultError> {
    list_import_attempts_page(conn, instrument_id, limit, before, None)
}

/// Cursor page of retained imports. The immutable attempt ID anchors both timestamp
/// and tie-breaker; clock rollback does not make equal-time rows unreachable.
pub fn list_import_attempts_page(
    conn: &Connection,
    instrument_id: Option<InstrumentId>,
    limit: u32,
    before: Option<UtcTimestamp>,
    after_import_attempt_id: Option<ImportAttemptId>,
) -> Result<Vec<ImportAttemptSummary>, ResultError> {
    if !(1..=500).contains(&limit) {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Request 1 to 500 imports per page.",
        ));
    }
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
         ORDER BY processed_at DESC, id DESC LIMIT ?5"
    ))?;
    Ok(statement
        .query_map(
            params![
                instrument_id,
                before,
                after_time,
                after_import_attempt_id,
                limit + 1
            ],
            attempt_summary,
        )?
        .collect::<Result<_, _>>()?)
}

/// The columns [`attempt_summary`] reads, in order, from `res_import_attempts`
/// (not aliased); the origin comes from the interfaces module's published view.
pub(crate) const ATTEMPT_SUMMARY_COLUMNS: &str = "id, receipt_id, attempt_no, instrument_id, \
     outcome, failure_code, rows_total, rows_new, rows_duplicate, rows_reprocessed, rows_qc, \
     rows_quarantined, rows_not_result, rows_set_aside, rows_retained, processed_at, \
     (SELECT o.origin FROM ifc_v_receipt_origins o \
      WHERE o.receipt_id = res_import_attempts.receipt_id)";

/// Gets one import attempt with its file facts and its quarantined and
/// set-aside clinical rows. The engine audits this read.
pub fn get_import_attempt(
    conn: &Connection,
    id: ImportAttemptId,
) -> Result<ImportAttemptDetail, ResultError> {
    let (summary, registration_id) = conn
        .query_row(
            &format!(
                "SELECT {ATTEMPT_SUMMARY_COLUMNS}, registration_id \
                 FROM res_import_attempts WHERE id = ?1"
            ),
            [id],
            |row| Ok((attempt_summary(row)?, row.get(17)?)),
        )
        .optional()?
        .ok_or(ResultError::NotFound("import attempt"))?;
    let quarantine = conn
        .prepare_cached(
            "SELECT id, record_path, sample_label, source_test_code, value_text, units_text, \
             reason, state, revision FROM res_quarantine_items \
             WHERE import_attempt_id = ?1 ORDER BY record_path, id",
        )?
        .query_map([id], |row| {
            Ok(QuarantineItemView {
                item_id: row.get(0)?,
                record_path: row.get(1)?,
                sample_label: row.get(2)?,
                test_code: row.get(3)?,
                value_text: row.get(4)?,
                units_text: row.get(5)?,
                reason: row.get(6)?,
                state: row.get(7)?,
                revision: row.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let set_aside = conn
        .prepare_cached(
            "SELECT id, record_path, sample_label, source_test_code, value_text, type_word, \
             kind, reason FROM res_set_aside_rows \
             WHERE import_attempt_id = ?1 ORDER BY record_path, id",
        )?
        .query_map([id], |row| {
            Ok(SetAsideRowView {
                row_id: row.get(0)?,
                record_path: row.get(1)?,
                sample_label: row.get(2)?,
                test_code: row.get(3)?,
                value_text: row.get(4)?,
                type_word: row.get(5)?,
                kind: row.get(6)?,
                reason: row.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(ImportAttemptDetail {
        file: import_file(conn, summary.receipt_id)?,
        summary,
        registration_id,
        quarantine,
        set_aside,
    })
}

/// The file facts of a result-file receipt, with the digest of its bytes.
fn import_file(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<Option<ImportFileView>, ResultError> {
    let Some(file) = lis_interfaces::receipt_file(conn, receipt_id)? else {
        return Ok(None);
    };
    let sha256 = conn
        .prepare_cached("SELECT message_sha256 FROM ifc_receipts WHERE id = ?1")?
        .query_row([receipt_id], |row| row.get(0))?;
    Ok(Some(ImportFileView {
        origin: file.origin,
        file_name: file.file_name,
        byte_len: file.byte_len,
        sha256,
        loaded_by: file.loaded_by,
        loaded_by_display: None,
    }))
}

pub(crate) fn attempt_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportAttemptSummary> {
    Ok(ImportAttemptSummary {
        import_attempt_id: row.get(0)?,
        receipt_id: row.get::<_, ReceiptId>(1)?,
        attempt_no: row.get(2)?,
        instrument_id: row.get(3)?,
        outcome: row.get(4)?,
        failure_code: row.get(5)?,
        rows_total: row.get(6)?,
        rows_new: row.get(7)?,
        rows_duplicate: row.get(8)?,
        rows_reprocessed: row.get(9)?,
        rows_qc: row.get(10)?,
        rows_quarantined: row.get(11)?,
        rows_not_result: row.get(12)?,
        rows_set_aside: row.get(13)?,
        rows_retained: row.get(14)?,
        processed_at: row.get(15)?,
        origin: row.get(16)?,
    })
}

/// The one home of every gate used to approve a technical review. It is safe
/// to call on a read connection for display or inside the review transaction
/// The latest attempt number for a receipt.
pub(crate) fn next_attempt_no(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<i64, ResultError> {
    let current: Option<i64> = conn.query_row(
        "SELECT max(attempt_no) FROM res_import_attempts WHERE receipt_id = ?1",
        [receipt_id],
        |row| row.get(0),
    )?;
    current
        .and_then(|attempt| attempt.checked_add(1))
        .ok_or(ResultError::WrongState("import receipt"))
}

/// Latest processing attempt for this exact durable receipt, independent of list page
/// and wall-clock rollback. `None` means received but not yet processed, not success.
pub fn latest_import_attempt_for_receipt(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<Option<ImportAttemptSummary>, ResultError> {
    Ok(conn.query_row(&format!("SELECT {ATTEMPT_SUMMARY_COLUMNS} FROM res_import_attempts WHERE receipt_id=?1 ORDER BY attempt_no DESC LIMIT 1"),[receipt_id],attempt_summary).optional()?)
}

#[cfg(test)]
mod paging_tests {
    use super::*;

    #[test]
    fn import_pages_reach_equal_times_and_clock_rollback_and_find_old_receipts()
    -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("CREATE TABLE res_import_attempts(id TEXT PRIMARY KEY,receipt_id TEXT,attempt_no INTEGER,instrument_id TEXT,outcome TEXT,failure_code TEXT,rows_total INTEGER,rows_new INTEGER,rows_duplicate INTEGER,rows_reprocessed INTEGER,rows_qc INTEGER,rows_quarantined INTEGER,rows_not_result INTEGER,rows_set_aside INTEGER,rows_retained INTEGER,processed_at TEXT);
        CREATE TABLE ifc_v_receipt_origins(receipt_id TEXT,origin TEXT);")?;
        let instrument = InstrumentId::generate();
        let old_receipt = ReceiptId::generate();
        let mut expected = Vec::new();
        for index in 0..503 {
            let id = ImportAttemptId::generate();
            let receipt = if index == 502 {
                old_receipt
            } else {
                ReceiptId::generate()
            };
            // The last persisted import follows a rolled-back clock, and is outside page 1.
            let time = if index == 502 {
                "2020-01-01T09:00:00.000000000Z"
            } else {
                "2026-10-14T09:00:00.000000000Z"
            };
            conn.execute("INSERT INTO res_import_attempts VALUES(?1,?2,1,?3,'processed',NULL,1,1,0,0,0,0,0,0,0,?4)",params![id,receipt,instrument,time])?;
            expected.push((time.to_owned(), id));
        }
        expected.sort_by(|a, b| b.cmp(a));
        let mut cursor = None;
        let mut seen = Vec::new();
        loop {
            let page = list_import_attempts_page(&conn, Some(instrument), 200, None, cursor)?;
            seen.extend(page.iter().take(200).map(|row| row.import_attempt_id));
            if page.len() <= 200 {
                break;
            }
            cursor = Some(page[199].import_attempt_id);
        }
        assert_eq!(seen, expected.iter().map(|(_, id)| *id).collect::<Vec<_>>());
        let exact = latest_import_attempt_for_receipt(&conn, old_receipt)?
            .expect("old receipt is reachable");
        assert_eq!(exact.import_attempt_id, *seen.last().unwrap());
        assert!(latest_import_attempt_for_receipt(&conn, ReceiptId::generate())?.is_none());
        assert!(list_import_attempts_page(&conn, None, 0, None, None).is_err());
        assert!(list_import_attempts_page(&conn, None, 501, None, None).is_err());
        Ok(())
    }
}
