//! Received order messages as log rows, and an incoming connection's counts (ticket #4555
//! step 3c). Transport says it was received; the application outcome is what it became.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::connection_views::LastAttempt;
use super::error::PartnerError;
use super::message_queries::{ConnectionRef, PartnerMessageRow, TransportOutcome};
use super::model::{MessageSource, PartnerConnectionId, PartnerMessageType};
use super::received::{PartnerReceivedId, ReceivedState};

/// Received messages, newest first, from `since`, of one connection or all.
pub(crate) fn received_rows(
    conn: &Connection,
    since: Option<UtcTimestamp>,
    connection_id: Option<PartnerConnectionId>,
    limit: u32,
) -> Result<Vec<PartnerMessageRow>, PartnerError> {
    conn.prepare_cached(
        "SELECT r.id, r.received_at, r.order_id, COALESCE(r.placer_order, r.file_name), \
         c.id, c.name, c.protocol, r.state, r.reason_code, r.moved_to \
         FROM ifc_partner_received r JOIN ifc_partner_connections c ON c.id = r.connection_id \
         WHERE (?1 IS NULL OR r.received_at >= ?1) AND (?2 IS NULL OR c.id = ?2) \
         ORDER BY r.received_at DESC LIMIT ?3",
    )?
    .query_map(params![since, connection_id, limit], |row| {
        let id: PartnerReceivedId = row.get(0)?;
        Ok(PartnerMessageRow {
            message_id: id.cast(),
            delivery_id: None,
            at: row.get(1)?,
            message_type: PartnerMessageType::Hl7Orders,
            source: MessageSource::Order,
            source_id: row.get(2)?,
            source_number: None,
            source_label: row.get(3)?,
            connection: Some(ConnectionRef {
                id: row.get(4)?,
                name: row.get(5)?,
                protocol: row.get(6)?,
            }),
            reason: None,
            transport: TransportOutcome::Received,
            outcome_unknown: false,
            waiting: None,
            attempts: 0,
            given_up: false,
            superseded: false,
            next_attempt_at: None,
            error_code: row.get(8)?,
            written_path: row.get(9)?,
            ack_code: None,
            ack_text: None,
            awaiting_ack: false,
            unrouted: false,
            resolution: None,
            allowed_actions: Vec::new(),
            received: Some(row.get(7)?),
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// An incoming connection's last message read, its messages waiting for a patient or a
/// person, and its refused ones.
pub(crate) fn intake_counts(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<(Option<LastAttempt>, u32, u32), PartnerError> {
    let last = conn
        .prepare_cached(
            "SELECT received_at, CASE WHEN state = 'rejected' THEN reason_code END \
             FROM ifc_partner_received WHERE connection_id = ?1 \
             ORDER BY received_at DESC LIMIT 1",
        )?
        .query_row([connection_id], |row| {
            Ok(LastAttempt {
                at: row.get(0)?,
                error_code: row.get(1)?,
            })
        })
        .optional()?;
    let (waiting, refused) = conn.query_row(
        "SELECT COALESCE(SUM(state IN (?2, ?3)), 0), COALESCE(SUM(state = ?4), 0) \
         FROM ifc_partner_received WHERE connection_id = ?1",
        params![
            connection_id,
            ReceivedState::WaitingPatient,
            ReceivedState::Held,
            ReceivedState::Rejected
        ],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((last, waiting, refused))
}
