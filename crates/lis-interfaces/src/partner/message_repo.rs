//! SQL for messages, deliveries and unrouted decisions.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::error::PartnerError;
use super::model::{
    DeliveryReason, DeliveryState, MessageSource, PartnerConnectionId, PartnerDeliveryId,
    PartnerMessageId, PartnerMessageType, RoutingFacts, UnroutedResolution,
};

/// A message as stored.
pub(crate) struct MessageRow {
    pub(crate) id: PartnerMessageId,
    pub(crate) message_type: PartnerMessageType,
    pub(crate) source: MessageSource,
    pub(crate) source_id: Option<String>,
    pub(crate) source_number: Option<i64>,
    /// The report's ID, for a report's revision or withdrawal.
    pub(crate) source_group: Option<String>,
    /// The report number, for a report's revision or withdrawal.
    pub(crate) source_label: Option<String>,
    pub(crate) artifact_sha256: String,
    pub(crate) facts: RoutingFacts,
    pub(crate) checked_against: i64,
    pub(crate) unrouted: bool,
    pub(crate) created_at: UtcTimestamp,
}

/// A delivery's latest state.
pub(crate) struct DeliveryRow {
    pub(crate) id: PartnerDeliveryId,
    pub(crate) message_id: PartnerMessageId,
    pub(crate) connection_id: PartnerConnectionId,
    pub(crate) reason: DeliveryReason,
    pub(crate) state: DeliveryState,
    pub(crate) attempts: i64,
}

/// A decision on an unrouted message.
pub(crate) struct ResolutionRow {
    pub(crate) resolution: UnroutedResolution,
    pub(crate) connection_id: Option<PartnerConnectionId>,
    pub(crate) note: Option<String>,
    pub(crate) resolved_by: String,
    pub(crate) resolved_at: UtcTimestamp,
}

/// SQL condition on message `m`: no newer revision or withdrawal of the same report
/// replaced it. Messages that are not about a report are never replaced.
pub(crate) const NOT_REPLACED: &str = "NOT EXISTS (SELECT 1 FROM ifc_partner_messages n \
     WHERE n.id <> m.id AND n.message_type = m.message_type \
     AND n.source_group = m.source_group \
     AND (n.source_kind = 'report_withdrawal' OR n.source_number > m.source_number))";

const MESSAGE_COLUMNS: &str = "m.id, m.message_type, m.source_kind, m.source_id, \
     m.source_number, m.artifact_sha256, m.routing_facts, m.checked_against, m.unrouted, \
     m.created_at, m.source_group, m.source_label";

pub(crate) fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), PartnerError> {
    conn.execute(
        "INSERT INTO ifc_partner_messages (id, message_type, source_kind, source_id, \
         source_number, artifact_sha256, routing_facts, checked_against, unrouted, created_at, \
         source_group, source_label) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            row.id,
            row.message_type,
            row.source,
            row.source_id,
            row.source_number,
            row.artifact_sha256,
            serde_json::to_string(&row.facts)?,
            row.checked_against,
            row.unrouted,
            row.created_at,
            row.source_group,
            row.source_label
        ],
    )?;
    Ok(())
}

pub(crate) fn load_message(
    conn: &Connection,
    id: PartnerMessageId,
) -> Result<Option<MessageRow>, PartnerError> {
    let row = conn
        .prepare_cached(&format!(
            "SELECT {MESSAGE_COLUMNS} FROM ifc_partner_messages m WHERE m.id = ?1"
        ))?
        .query_row([id], message_columns)
        .optional()?;
    row.map(with_facts).transpose()
}

/// Whether a newer revision or a withdrawal of the same report replaced `message`.
pub(crate) fn replaced(conn: &Connection, id: PartnerMessageId) -> Result<bool, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT NOT {NOT_REPLACED} FROM ifc_partner_messages m WHERE m.id = ?1"
    ))?
    .query_row([id], |row| row.get(0))
    .map_err(Into::into)
}

/// Unrouted messages without a decision and not replaced, oldest first.
pub(crate) fn open_unrouted(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<MessageRow>, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM ifc_partner_messages m \
         WHERE m.unrouted = 1 AND NOT EXISTS \
         (SELECT 1 FROM ifc_partner_unrouted_resolutions r WHERE r.message_id = m.id) \
         AND {NOT_REPLACED} ORDER BY m.created_at LIMIT ?1"
    ))?
    .query_map([limit], message_columns)?
    .map(|row| with_facts(row?))
    .collect()
}

/// The connections that have an earlier message of `report_id` of `message_type`, with
/// whether it was written there. Superseded deliveries do not count.
pub(crate) fn report_recipients(
    conn: &Connection,
    message_type: PartnerMessageType,
    report_id: &str,
) -> Result<Vec<(PartnerConnectionId, bool)>, PartnerError> {
    conn.prepare_cached(
        "SELECT d.connection_id, MAX(d.state = 'written') FROM ifc_partner_deliveries d \
         JOIN ifc_partner_messages m ON m.id = d.message_id \
         WHERE m.message_type = ?1 AND m.source_group = ?2 AND d.state <> 'superseded' \
         GROUP BY d.connection_id ORDER BY d.connection_id",
    )?
    .query_map(params![message_type, report_id], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// Earlier messages of `report_id` not written yet are superseded: never written now.
/// Returns how many deliveries that stopped.
pub(crate) fn supersede_unwritten(
    conn: &Connection,
    message_type: PartnerMessageType,
    report_id: &str,
) -> Result<usize, PartnerError> {
    conn.execute(
        "UPDATE ifc_partner_deliveries SET state = 'superseded', next_attempt_at = NULL \
         WHERE state IN ('waiting', 'failed') AND message_id IN \
         (SELECT id FROM ifc_partner_messages WHERE message_type = ?1 AND source_group = ?2)",
        params![message_type, report_id],
    )
    .map_err(Into::into)
}

type MessageColumns = (MessageRow, String);

fn message_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageColumns> {
    Ok((
        MessageRow {
            id: row.get(0)?,
            message_type: row.get(1)?,
            source: row.get(2)?,
            source_id: row.get(3)?,
            source_number: row.get(4)?,
            source_group: row.get(10)?,
            source_label: row.get(11)?,
            artifact_sha256: row.get(5)?,
            facts: RoutingFacts::default(),
            checked_against: row.get(7)?,
            unrouted: row.get(8)?,
            created_at: row.get(9)?,
        },
        row.get(6)?,
    ))
}

fn with_facts((mut row, facts): MessageColumns) -> Result<MessageRow, PartnerError> {
    row.facts = serde_json::from_str(&facts)?;
    Ok(row)
}

/// A new delivery, waiting for the poller.
pub(crate) fn insert_delivery(
    conn: &Connection,
    message_id: PartnerMessageId,
    connection_id: PartnerConnectionId,
    reason: DeliveryReason,
    now: UtcTimestamp,
) -> Result<PartnerDeliveryId, PartnerError> {
    let id = PartnerDeliveryId::generate();
    conn.execute(
        "INSERT INTO ifc_partner_deliveries \
         (id, message_id, connection_id, reason, created_at, state, attempts) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'waiting', 0)",
        params![id, message_id, connection_id, reason, now],
    )?;
    Ok(id)
}

pub(crate) fn load_delivery(
    conn: &Connection,
    id: PartnerDeliveryId,
) -> Result<Option<DeliveryRow>, PartnerError> {
    conn.prepare_cached(
        "SELECT id, message_id, connection_id, reason, state, attempts \
         FROM ifc_partner_deliveries WHERE id = ?1",
    )?
    .query_row([id], |row| {
        Ok(DeliveryRow {
            id: row.get(0)?,
            message_id: row.get(1)?,
            connection_id: row.get(2)?,
            reason: row.get(3)?,
            state: row.get(4)?,
            attempts: row.get(5)?,
        })
    })
    .optional()
    .map_err(Into::into)
}

/// A written file: the delivery is done.
pub(crate) fn set_written(
    conn: &Connection,
    id: PartnerDeliveryId,
    path: &str,
    now: UtcTimestamp,
) -> Result<(), PartnerError> {
    conn.execute(
        "UPDATE ifc_partner_deliveries SET state = 'written', written_path = ?2, \
         written_at = ?3, last_attempt_at = ?3, last_error_code = NULL, next_attempt_at = NULL \
         WHERE id = ?1",
        params![id, path, now],
    )?;
    Ok(())
}

/// A failed attempt: retried at `retry_at`, or failed for good when `None`.
pub(crate) fn set_failed_attempt(
    conn: &Connection,
    id: PartnerDeliveryId,
    error_code: &str,
    now: UtcTimestamp,
    retry_at: Option<UtcTimestamp>,
) -> Result<(), PartnerError> {
    conn.execute(
        "UPDATE ifc_partner_deliveries SET attempts = attempts + 1, last_attempt_at = ?3, \
         last_error_code = ?2, next_attempt_at = ?4, \
         state = CASE WHEN ?4 IS NULL THEN 'failed' ELSE 'waiting' END WHERE id = ?1",
        params![id, error_code, now, retry_at],
    )?;
    Ok(())
}

/// Keeps the bytes a waiting delivery made; returns 0 when it is not waiting or has some.
pub(crate) fn set_payload(
    conn: &Connection,
    id: PartnerDeliveryId,
    payload_sha256: &str,
    mapping_version: i64,
) -> Result<usize, PartnerError> {
    conn.execute(
        "UPDATE ifc_partner_deliveries SET payload_sha256 = ?2, mapping_version = ?3 \
         WHERE id = ?1 AND state = 'waiting' AND payload_sha256 IS NULL",
        params![id, payload_sha256, mapping_version],
    )
    .map_err(Into::into)
}

/// A failed delivery waits again, with a fresh set of attempts.
pub(crate) fn set_waiting_again(
    conn: &Connection,
    id: PartnerDeliveryId,
) -> Result<(), PartnerError> {
    conn.execute(
        "UPDATE ifc_partner_deliveries SET state = 'waiting', attempts = 0, \
         next_attempt_at = NULL WHERE id = ?1 AND state = 'failed'",
        [id],
    )?;
    Ok(())
}

pub(crate) fn resolution(
    conn: &Connection,
    message_id: PartnerMessageId,
) -> Result<Option<UnroutedResolution>, PartnerError> {
    conn.prepare_cached(
        "SELECT resolution FROM ifc_partner_unrouted_resolutions WHERE message_id = ?1",
    )?
    .query_row([message_id], |row| row.get(0))
    .optional()
    .map_err(Into::into)
}

pub(crate) fn insert_resolution(
    conn: &Connection,
    message_id: PartnerMessageId,
    row: &ResolutionRow,
) -> Result<(), PartnerError> {
    conn.execute(
        "INSERT INTO ifc_partner_unrouted_resolutions \
         (message_id, resolution, connection_id, note, resolved_by, resolved_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            message_id,
            row.resolution,
            row.connection_id,
            row.note,
            row.resolved_by,
            row.resolved_at
        ],
    )?;
    Ok(())
}
