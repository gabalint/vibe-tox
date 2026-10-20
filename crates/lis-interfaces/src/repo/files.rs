//! SQL for result-file receipts, their file facts, the ledger of files already
//! received, the files present when a folder was chosen, and the watched
//! folder (result-files brief F1, F13, §6).

use lis_core::{InstrumentId, Phi, UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::InterfaceError;
use crate::model::{
    FileFacts, FileOrigin, PresentFile, Receipt, ReceiptFileView, ReceiptId, ReceiptState,
};

/// Inserts a terminal result-file receipt and its file facts.
pub(crate) fn insert_file_receipt(
    conn: &Connection,
    receipt: &Receipt,
    file: &FileFacts,
    loaded_by: Option<UserId>,
) -> Result<(), InterfaceError> {
    let state = match receipt.state {
        ReceiptState::Complete => "complete",
        ReceiptState::Aborted => "aborted",
        ReceiptState::Receiving | ReceiptState::Interrupted => {
            return Err(InterfaceError::InvalidReceiptState);
        }
    };
    conn.execute(
        "INSERT INTO ifc_receipts (
            id, instrument_id, transport, peer, started_at, completed_at, state,
            abort_reason, frame_count, message_sha256, artifact_digest, duplicate_of
         ) VALUES (?, ?, 'result_file', NULL, ?, ?, ?, ?, 0, ?, ?, ?)",
        params![
            receipt.id,
            receipt.instrument_id,
            receipt.started_at,
            receipt.completed_at,
            state,
            receipt.abort_reason.map(|reason| reason.as_str()),
            receipt.message_sha256,
            receipt.artifact_digest,
            receipt.duplicate_of,
        ],
    )?;
    conn.execute(
        "INSERT INTO ifc_receipt_files (
            receipt_id, origin, file_name, byte_len, modified_at, loaded_by
         ) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            receipt.id,
            file.source.origin().as_str(),
            file.file_name.expose(),
            sql_len(file.byte_len)?,
            file.modified_at,
            loaded_by,
        ],
    )?;
    Ok(())
}

/// Adds the file to the ledger of files already received, under `receipt_id`.
pub(crate) fn insert_seen_file(
    conn: &Connection,
    file: &FileFacts,
    receipt_id: ReceiptId,
) -> Result<(), InterfaceError> {
    conn.execute(
        "INSERT INTO ifc_seen_files (
            instrument_id, file_name, byte_len, modified_at, receipt_id
         ) VALUES (?, ?, ?, ?, ?)",
        params![
            file.instrument_id,
            file.file_name.expose(),
            sql_len(file.byte_len)?,
            file.modified_at,
            receipt_id,
        ],
    )?;
    Ok(())
}

/// The receipt of the file with this name, size and modification time, if
/// one was received for the instrument.
pub(crate) fn seen_receipt(
    conn: &Connection,
    instrument_id: InstrumentId,
    file_name: &str,
    byte_len: u64,
    modified_at: UtcTimestamp,
) -> Result<Option<ReceiptId>, InterfaceError> {
    conn.prepare_cached(
        "SELECT receipt_id FROM ifc_seen_files
         WHERE instrument_id = ? AND file_name = ? AND byte_len = ? AND modified_at = ?",
    )?
    .query_row(
        params![instrument_id, file_name, sql_len(byte_len)?, modified_at],
        |row| row.get(0),
    )
    .optional()
    .map_err(InterfaceError::from)
}

/// Records a file present in `folder` when the instrument's folder was chosen;
/// a file already recorded for that folder is kept as it is.
pub(crate) fn insert_baseline_file(
    conn: &Connection,
    instrument_id: InstrumentId,
    folder: &str,
    file: &PresentFile,
    recorded_at: UtcTimestamp,
) -> Result<(), InterfaceError> {
    conn.prepare_cached(
        "INSERT OR IGNORE INTO ifc_folder_baseline_files (
            instrument_id, folder, file_name, byte_len, modified_at, recorded_at
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )?
    .execute(params![
        instrument_id,
        folder,
        file.file_name.expose(),
        sql_len(file.byte_len)?,
        file.modified_at,
        recorded_at,
    ])?;
    Ok(())
}

/// Whether the file with this name, size and modification time was in
/// `folder` when the instrument's folder was chosen.
pub(crate) fn in_baseline(
    conn: &Connection,
    instrument_id: InstrumentId,
    folder: &str,
    file_name: &str,
    byte_len: u64,
    modified_at: UtcTimestamp,
) -> Result<bool, InterfaceError> {
    Ok(conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM ifc_folder_baseline_files
             WHERE instrument_id = ? AND folder = ? AND file_name = ? AND byte_len = ?
               AND modified_at = ?)",
        )?
        .query_row(
            params![
                instrument_id,
                folder,
                file_name,
                sql_len(byte_len)?,
                modified_at
            ],
            |row| row.get(0),
        )?)
}

pub(crate) fn receipt_file(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<Option<ReceiptFileView>, InterfaceError> {
    conn.prepare_cached(
        "SELECT receipt_id, origin, file_name, byte_len, modified_at, loaded_by
         FROM ifc_receipt_files WHERE receipt_id = ?",
    )?
    .query_row([receipt_id], |row| {
        let stored_len: i64 = row.get(3)?;
        Ok(ReceiptFileView {
            receipt_id: row.get(0)?,
            origin: FileOrigin::parse(row.get_ref(1)?.as_str()?)?,
            file_name: Phi::new(row.get(2)?),
            byte_len: u64::try_from(stored_len)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, stored_len))?,
            modified_at: row.get(4)?,
            loaded_by: row.get(5)?,
        })
    })
    .optional()
    .map_err(InterfaceError::from)
}

/// A file size as SQLite stores integers.
fn sql_len(byte_len: u64) -> Result<i64, InterfaceError> {
    i64::try_from(byte_len).map_err(|_| InterfaceError::InvalidFileSize)
}

/// Whether another instrument watches `folder`.
pub(crate) fn result_folder_in_use(
    conn: &Connection,
    folder: &str,
    excluding: InstrumentId,
) -> Result<bool, InterfaceError> {
    let found: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM ifc_instruments WHERE result_folder = ?1 AND id <> ?2)",
        params![folder, excluding],
        |row| row.get(0),
    )?;
    Ok(found != 0)
}

pub(crate) fn update_result_folder(
    conn: &Connection,
    instrument_id: InstrumentId,
    folder: Option<&str>,
    next_revision: i64,
) -> Result<(), InterfaceError> {
    conn.execute(
        "UPDATE ifc_instruments SET result_folder = ?, revision = ? WHERE id = ?",
        params![folder, next_revision, instrument_id],
    )?;
    Ok(())
}
