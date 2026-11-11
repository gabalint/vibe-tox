//! Import-attempt summary columns and the audited-detail read model.

use super::*;

/// The columns [`attempt_summary`] reads, in order, from `res_import_attempts`
/// (not aliased); the origin and received time come from the interfaces module's
/// published views, and a duplicate's earlier attempt is the latest one of the
/// receipt it repeats.
pub(crate) const ATTEMPT_SUMMARY_COLUMNS: &str = "id, receipt_id, attempt_no, instrument_id, \
     outcome, failure_code, rows_total, rows_new, rows_duplicate, rows_reprocessed, rows_qc, \
     rows_quarantined, rows_not_result, rows_set_aside, rows_retained, processed_at, \
     (SELECT o.origin FROM ifc_v_receipt_origins o \
      WHERE o.receipt_id = res_import_attempts.receipt_id), \
     (SELECT a.received_at FROM ifc_v_receipt_arrivals a \
      WHERE a.receipt_id = res_import_attempts.receipt_id), \
     (SELECT d.id FROM res_import_attempts d \
      WHERE d.receipt_id = res_import_attempts.duplicate_of_receipt_id \
      ORDER BY d.attempt_no DESC LIMIT 1)";

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
            |row| Ok((attempt_summary(row)?, row.get(19)?)),
        )
        .optional()?
        .ok_or(ResultError::NotFound("import attempt"))?;
    let quarantine = super::quarantine_items::quarantine_items(conn, id, summary.instrument_id)?;
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
        received_at: row.get(17)?,
        duplicate_of_attempt_id: row.get(18)?,
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
