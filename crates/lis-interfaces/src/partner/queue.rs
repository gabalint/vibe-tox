//! Owned work for the work queues (REQ-RPT-003): unrouted messages waiting for a decision
//! and deliveries whose attempts all failed.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, params};

use super::error::PartnerError;
use super::model::{PartnerDeliveryId, PartnerMessageId};

/// One unrouted message waiting for a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnroutedItem {
    pub message_id: PartnerMessageId,
    /// The handoff number.
    pub source_number: Option<i64>,
    pub since: UtcTimestamp,
}

/// One delivery whose attempts all failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedItem {
    pub delivery_id: PartnerDeliveryId,
    pub source_number: Option<i64>,
    pub connection_name: String,
    pub since: UtcTimestamp,
}

/// Unrouted messages without a decision, oldest first.
pub fn unrouted_queue(conn: &Connection, limit: u32) -> Result<Vec<UnroutedItem>, PartnerError> {
    conn.prepare_cached(
        "SELECT m.id, m.source_number, m.created_at FROM ifc_partner_messages m \
         WHERE m.unrouted = 1 AND NOT EXISTS \
         (SELECT 1 FROM ifc_partner_unrouted_resolutions r WHERE r.message_id = m.id) \
         ORDER BY m.created_at LIMIT ?1",
    )?
    .query_map([limit], |row| {
        Ok(UnroutedItem {
            message_id: row.get(0)?,
            source_number: row.get(1)?,
            since: row.get(2)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// Deliveries that failed for good and wait to be sent again, oldest first. Test
/// messages are not work.
pub fn failed_queue(conn: &Connection, limit: u32) -> Result<Vec<FailedItem>, PartnerError> {
    conn.prepare_cached(
        "SELECT d.id, m.source_number, c.name, d.last_attempt_at \
         FROM ifc_partner_deliveries d \
         JOIN ifc_partner_messages m ON m.id = d.message_id \
         JOIN ifc_partner_connections c ON c.id = d.connection_id \
         WHERE d.state = 'failed' AND d.reason <> 'test' \
         ORDER BY d.last_attempt_at LIMIT ?1",
    )?
    .query_map(params![limit], |row| {
        Ok(FailedItem {
            delivery_id: row.get(0)?,
            source_number: row.get(1)?,
            connection_name: row.get(2)?,
            since: row.get(3)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}
