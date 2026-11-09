//! One outgoing message's details for the Logs panel (ticket #4678; REQ-INT-002,
//! REQ-RPT-011): the HL7 message as sent, its control ID (MSH-10) and every attempt.
//!
//! The content is never made again: an HL7 Results delivery keeps the bytes it made at its
//! first attempt in the artifact store, and every attempt sends those bytes, so they are
//! what the partner received. This module names the kept digest; the engine reads and
//! verifies the bytes, then [`PartnerMessageDetail::show`] turns them into segments. The
//! attempts are the delivery's `interfaces.write_message` audit events.

use lis_audit::{EventFilter, Stream};
use lis_core::{UtcTimestamp, text_enum};
use lis_store::StoreError;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::error::PartnerError;
use super::hl7_read::message_text;
use super::mllp::control_id;
use super::model::{MessageSource, PartnerDeliveryId, PartnerMessageType};

/// Attempts read at most.
const ATTEMPT_LIMIT: u32 = 100;
/// The audit action of each attempt (see `record_delivery_attempt`).
const WRITE_ACTION: &str = "interfaces.write_message";

/// Input of `get_partner_message`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetPartnerMessage {
    pub delivery_id: PartnerDeliveryId,
}

text_enum! {
    /// What the panel can show of the message itself.
    #[ts(export)]
    MessageContent {
        /// The HL7 message as sent: the kept bytes, which matched their recorded digest.
        Sent = "sent",
        /// Not made yet: it waits for its first attempt or for a code mapping.
        NotMade = "not_made",
        /// A report PDF or billing file, not an HL7 message.
        File = "file",
        /// The kept bytes are missing or no longer match their digest; nothing is shown.
        Unavailable = "unavailable",
    }
}

/// One attempt to send the message, as its audit event recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerAttempt {
    pub at: UtcTimestamp,
    /// Written, or sent and answered; false when it failed.
    pub succeeded: bool,
    /// Why it failed, such as `no_ack` or `connection_refused`.
    pub error_code: Option<String>,
    /// Over MLLP: the partner's MSA-1 code, `AA`, `AE` or `AR`.
    pub ack_code: Option<String>,
}

/// One outgoing message: its control ID, its HL7 segments as sent, and its attempts, oldest
/// first. A synthetic test message has no attempt events: it is sent once, never retried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerMessageDetail {
    pub delivery_id: PartnerDeliveryId,
    /// MSH-10 of the message as sent; `None` until it is made, or for a file.
    pub control_id: Option<String>,
    pub content: MessageContent,
    /// The segments as sent, one per line; empty unless `content` is `sent`.
    pub segments: Vec<String>,
    pub attempts: Vec<PartnerAttempt>,
    /// The digest of the bytes to show, for the engine; never sent to the UI.
    #[serde(skip)]
    #[ts(skip)]
    pub kept_sha256: Option<String>,
}

impl PartnerMessageDetail {
    /// Shows `bytes`, read and verified against [`Self::kept_sha256`]: their segments and
    /// MSH-10. Bytes that are not HL7 text stay unavailable.
    pub fn show(&mut self, bytes: &[u8]) {
        let Some(text) = message_text(bytes) else {
            return;
        };
        self.segments = text
            .split(['\r', '\n'])
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect();
        self.control_id = control_id(bytes);
        self.content = MessageContent::Sent;
    }
}

/// The delivery's details, with the kept bytes still to read.
pub fn partner_message_detail(
    conn: &Connection,
    delivery_id: PartnerDeliveryId,
) -> Result<PartnerMessageDetail, PartnerError> {
    let (message_type, source, artifact, payload): (
        PartnerMessageType,
        MessageSource,
        String,
        Option<String>,
    ) = conn
        .prepare_cached(
            "SELECT m.message_type, m.source_kind, m.artifact_sha256, d.payload_sha256 \
             FROM ifc_partner_deliveries d JOIN ifc_partner_messages m ON m.id = d.message_id \
             WHERE d.id = ?1",
        )?
        .query_row([delivery_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()?
        .ok_or(PartnerError::NotFound("delivery"))?;
    let kept_sha256 = match message_type {
        // A synthetic test message is stored whole when it is sent.
        PartnerMessageType::Hl7Oru if source == MessageSource::Test => Some(artifact),
        PartnerMessageType::Hl7Oru => payload,
        PartnerMessageType::Hl7Orders
        | PartnerMessageType::ReportPdf
        | PartnerMessageType::BillingJson => None,
    };
    let content = match (message_type, &kept_sha256) {
        (PartnerMessageType::Hl7Oru, Some(_)) => MessageContent::Unavailable,
        (PartnerMessageType::Hl7Oru, None) => MessageContent::NotMade,
        _ => MessageContent::File,
    };
    Ok(PartnerMessageDetail {
        delivery_id,
        control_id: None,
        content,
        segments: Vec::new(),
        attempts: attempts(conn, delivery_id)?,
        kept_sha256,
    })
}

/// The delivery's attempts, oldest first, from its audit events.
fn attempts(
    conn: &Connection,
    delivery_id: PartnerDeliveryId,
) -> Result<Vec<PartnerAttempt>, PartnerError> {
    let record_id = delivery_id.to_string();
    let filter = EventFilter {
        action: Some(WRITE_ACTION),
        record_type: Some("partner_delivery"),
        record_id: Some(&record_id),
        ..EventFilter::new(Stream::Security, ATTEMPT_LIMIT)
    };
    let events = lis_audit::list(conn, &filter).map_err(StoreError::from)?;
    let mut attempts = Vec::with_capacity(events.len());
    for event in events.into_iter().rev() {
        let details = lis_audit::event_facts(conn, event.stream, event.seq)
            .map_err(StoreError::from)?
            .map(|facts| facts.details)
            .unwrap_or_default();
        let text = |key: &str| details.get(key).and_then(Value::as_str).map(str::to_owned);
        attempts.push(PartnerAttempt {
            at: event.occurred_at,
            succeeded: event.outcome == "succeeded",
            error_code: text("error_code"),
            ack_code: text("ack_code"),
        });
    }
    Ok(attempts)
}

#[cfg(test)]
#[path = "message_detail_tests.rs"]
mod tests;
