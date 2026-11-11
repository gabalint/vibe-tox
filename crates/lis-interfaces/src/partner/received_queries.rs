//! Received order messages as log rows, and an incoming connection's counts (ticket #4555
//! step 3c). Transport says it was received; the application outcome is what it became.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::connection_views::LastAttempt;
use super::error::PartnerError;
use super::message_queries::{ConnectionRef, PartnerMessageRow, TransportOutcome, allowed};
use super::model::{MessageSource, PartnerConnectionId, PartnerMessageType};
use super::received::{PartnerReceivedId, ReceivedState};
use super::rejections::{RESOLUTION_COLUMNS, resolution_at};

/// Received messages, newest first, from `since`, of one connection or all.
pub(crate) fn received_rows(
    conn: &Connection,
    since: Option<UtcTimestamp>,
    connection_id: Option<PartnerConnectionId>,
    limit: u32,
) -> Result<Vec<PartnerMessageRow>, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT r.id, r.received_at, r.order_id, \
         COALESCE(r.placer_order, NULLIF(r.file_name, '')), \
         c.id, c.name, c.protocol, r.state, r.reason_code, r.moved_to, r.ack_code, r.peer, \
         {RESOLUTION_COLUMNS} \
         FROM ifc_partner_received r JOIN ifc_partner_connections c ON c.id = r.connection_id \
         LEFT JOIN ifc_partner_rejection_resolutions x ON x.received_id = r.id \
         WHERE (?1 IS NULL OR r.received_at >= ?1) AND (?2 IS NULL OR c.id = ?2) \
         ORDER BY r.received_at DESC LIMIT ?3"
    ))?
    .query_map(params![since, connection_id, limit], |row| {
        let id: PartnerReceivedId = row.get(0)?;
        let state: ReceivedState = row.get(7)?;
        let rejection_resolved = resolution_at(row, 12)?;
        let actions = [
            (
                matches!(state, ReceivedState::WaitingPatient | ReceivedState::Held),
                "reject_received_message",
            ),
            (
                state == ReceivedState::Rejected && rejection_resolved.is_none(),
                "resolve_partner_rejection",
            ),
        ];
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
            ack_code: row.get(10)?,
            ack_text: None,
            awaiting_ack: false,
            peer: row.get(11)?,
            unrouted: false,
            resolution: None,
            rejection_resolved,
            allowed_actions: allowed(&actions),
            received: Some(state),
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
    // A refused message is the partner's content, not a failed connection: the engine adds
    // a folder's or server's reading problems, and a listener's.
    let last = conn
        .prepare_cached(
            "SELECT received_at FROM ifc_partner_received WHERE connection_id = ?1 \
             ORDER BY received_at DESC LIMIT 1",
        )?
        .query_row([connection_id], |row| {
            Ok(LastAttempt {
                at: row.get(0)?,
                error_code: None,
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
