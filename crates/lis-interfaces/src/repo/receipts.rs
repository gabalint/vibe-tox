//! SQL for durable receipts and their exact frames (REQ-INT-009).

use lis_core::{InstrumentId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::InterfaceError;
use crate::model::{
    AbortCode, InstrumentTransport, Receipt, ReceiptFrame, ReceiptId, ReceiptState,
};

pub(crate) fn insert_receipt(conn: &Connection, receipt: &Receipt) -> Result<(), InterfaceError> {
    conn.execute(
        "INSERT INTO ifc_receipts (
            id, instrument_id, transport, peer, started_at, completed_at, state,
            abort_reason, frame_count, message_sha256, artifact_digest, duplicate_of
         ) VALUES (?, ?, ?, ?, ?, NULL, 'receiving', NULL, 0, NULL, NULL, NULL)",
        params![
            receipt.id,
            receipt.instrument_id,
            receipt.transport.as_str(),
            receipt.peer,
            receipt.started_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn insert_frame(conn: &Connection, frame: &ReceiptFrame) -> Result<(), InterfaceError> {
    conn.execute(
        "INSERT INTO ifc_receipt_frames (receipt_id, ordinal, bytes) VALUES (?, ?, ?)",
        params![frame.receipt_id, frame.ordinal, frame.bytes],
    )?;
    conn.execute(
        "UPDATE ifc_receipts SET frame_count = frame_count + 1 WHERE id = ?",
        [frame.receipt_id],
    )?;
    Ok(())
}

pub(crate) fn complete_receipt(
    conn: &Connection,
    receipt_id: ReceiptId,
    completed_at: UtcTimestamp,
    digest: &str,
    duplicate_of: Option<ReceiptId>,
) -> Result<(), InterfaceError> {
    conn.execute(
        "UPDATE ifc_receipts SET state = 'complete', completed_at = ?,
            message_sha256 = ?, artifact_digest = ?, duplicate_of = ?
         WHERE id = ? AND state = 'receiving'",
        params![completed_at, digest, digest, duplicate_of, receipt_id],
    )?;
    Ok(())
}

pub(crate) fn terminate_receipt(
    conn: &Connection,
    receipt_id: ReceiptId,
    state: ReceiptState,
    completed_at: UtcTimestamp,
    reason: AbortCode,
) -> Result<(), InterfaceError> {
    let state = match state {
        ReceiptState::Aborted => "aborted",
        ReceiptState::Interrupted => "interrupted",
        ReceiptState::Receiving | ReceiptState::Complete => {
            return Err(InterfaceError::InvalidReceiptState);
        }
    };
    conn.execute(
        "UPDATE ifc_receipts SET state = ?, completed_at = ?, abort_reason = ?
         WHERE id = ? AND state = 'receiving'",
        params![state, completed_at, reason.as_str(), receipt_id],
    )?;
    Ok(())
}

pub(crate) fn receipt_by_id(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<Option<Receipt>, InterfaceError> {
    conn.query_row(
        "SELECT id, instrument_id, transport, peer, started_at, completed_at, state,
                abort_reason, frame_count, message_sha256, artifact_digest, duplicate_of
         FROM ifc_receipts WHERE id = ?",
        [receipt_id],
        map_receipt,
    )
    .optional()
    .map_err(InterfaceError::from)
}

pub(crate) fn frames_for_receipt(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<Vec<ReceiptFrame>, InterfaceError> {
    let mut statement = conn.prepare_cached(
        "SELECT receipt_id, ordinal, bytes FROM ifc_receipt_frames
         WHERE receipt_id = ? ORDER BY ordinal",
    )?;
    let rows = statement.query_map([receipt_id], |row| {
        Ok(ReceiptFrame {
            receipt_id: row.get(0)?,
            ordinal: row.get(1)?,
            bytes: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(InterfaceError::from)
}

pub(crate) fn first_duplicate(
    conn: &Connection,
    instrument_id: InstrumentId,
    digest: &str,
) -> Result<Option<ReceiptId>, InterfaceError> {
    conn.query_row(
        "SELECT id FROM ifc_receipts
         WHERE instrument_id = ? AND message_sha256 = ? AND state = 'complete'
         AND transport NOT IN ('au_tcp', 'au_serial')
         ORDER BY completed_at, id LIMIT 1",
        params![instrument_id, digest],
        |row| row.get(0),
    )
    .optional()
    .map_err(InterfaceError::from)
}

/// Identical AU bytes are duplicates only under the same frozen wire profile.
/// A corrected configuration must be able to receive previously unreadable bytes.
pub(crate) fn first_au_duplicate(
    conn: &Connection,
    receipt: &Receipt,
    digest: &str,
) -> Result<Option<ReceiptId>, InterfaceError> {
    let profile =
        super::au_profile(conn, receipt.id)?.ok_or(InterfaceError::InvalidAuConfiguration)?;
    let mut statement = conn.prepare_cached(
        "SELECT id FROM ifc_receipts
         WHERE instrument_id = ? AND message_sha256 = ? AND state = 'complete'
         AND transport IN ('au_tcp', 'au_serial')
         ORDER BY completed_at, id",
    )?;
    let candidates =
        statement.query_map(params![receipt.instrument_id, digest], |row| row.get(0))?;
    for candidate in candidates {
        let id = candidate?;
        let previous =
            super::au_profile(conn, id)?.ok_or(InterfaceError::InvalidAuConfiguration)?;
        if previous == profile {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

pub(crate) fn receiving_receipts(conn: &Connection) -> Result<Vec<ReceiptId>, InterfaceError> {
    let mut statement = conn.prepare_cached(
        "SELECT id FROM ifc_receipts WHERE state = 'receiving' ORDER BY started_at, id",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(InterfaceError::from)
}

fn map_receipt(row: &Row<'_>) -> rusqlite::Result<Receipt> {
    let abort_reason = row
        .get::<_, Option<String>>(7)?
        .map(|value| AbortCode::parse(&value))
        .transpose()?;
    Ok(Receipt {
        id: row.get(0)?,
        instrument_id: row.get(1)?,
        transport: InstrumentTransport::parse(row.get_ref(2)?.as_str()?)?,
        peer: row.get(3)?,
        started_at: row.get(4)?,
        completed_at: row.get(5)?,
        state: ReceiptState::parse(row.get_ref(6)?.as_str()?)?,
        abort_reason,
        frame_count: row.get(8)?,
        message_sha256: row.get(9)?,
        artifact_digest: row.get(10)?,
        duplicate_of: row.get(11)?,
    })
}
