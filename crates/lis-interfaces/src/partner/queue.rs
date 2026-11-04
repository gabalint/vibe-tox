//! Owned work for the work queues (REQ-RPT-003): unrouted messages waiting for a decision
//! and deliveries whose attempts all failed.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, params};

use super::error::PartnerError;
use super::message_repo::NOT_REPLACED;
use super::model::{MessageSource, PartnerDeliveryId, PartnerMessageId};

/// One unrouted message waiting for a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnroutedItem {
    pub message_id: PartnerMessageId,
    /// The record it is about, such as "Billing handoff 12" or "A26000005-U r1".
    pub record: String,
    pub since: UtcTimestamp,
}

/// One delivery whose attempts all failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedItem {
    pub delivery_id: PartnerDeliveryId,
    pub record: String,
    pub connection_name: String,
    pub since: UtcTimestamp,
}

/// Unrouted messages without a decision, and not replaced by a newer revision,
/// oldest first.
pub fn unrouted_queue(conn: &Connection, limit: u32) -> Result<Vec<UnroutedItem>, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT m.id, m.source_kind, m.source_number, m.source_label, m.created_at \
         FROM ifc_partner_messages m \
         WHERE m.unrouted = 1 AND NOT EXISTS \
         (SELECT 1 FROM ifc_partner_unrouted_resolutions r WHERE r.message_id = m.id) \
         AND {NOT_REPLACED} ORDER BY m.created_at LIMIT ?1"
    ))?
    .query_map([limit], |row| {
        Ok(UnroutedItem {
            message_id: row.get(0)?,
            record: record_text(row.get(1)?, row.get(2)?, row.get(3)?),
            since: row.get(4)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// Deliveries that failed for good and wait to be sent again, oldest first. Test
/// messages are not work.
pub fn failed_queue(conn: &Connection, limit: u32) -> Result<Vec<FailedItem>, PartnerError> {
    conn.prepare_cached(
        "SELECT d.id, m.source_kind, m.source_number, m.source_label, c.name, d.last_attempt_at \
         FROM ifc_partner_deliveries d \
         JOIN ifc_partner_messages m ON m.id = d.message_id \
         JOIN ifc_partner_connections c ON c.id = d.connection_id \
         WHERE d.state = 'failed' AND d.reason <> 'test' \
         ORDER BY d.last_attempt_at LIMIT ?1",
    )?
    .query_map(params![limit], |row| {
        Ok(FailedItem {
            delivery_id: row.get(0)?,
            record: record_text(row.get(1)?, row.get(2)?, row.get(3)?),
            connection_name: row.get(4)?,
            since: row.get(5)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// The record a message is about, as the UI's Logs name it.
fn record_text(source: MessageSource, number: Option<i64>, label: Option<String>) -> String {
    let number = number.unwrap_or_default();
    let label = label.unwrap_or_default();
    match source {
        MessageSource::BillingHandoff => format!("Billing handoff {number}"),
        MessageSource::ReportRevision => format!("{label} r{number}"),
        MessageSource::ReportWithdrawal => format!("{label} withdrawal notice"),
        MessageSource::Test => "Test message".to_owned(),
    }
}
