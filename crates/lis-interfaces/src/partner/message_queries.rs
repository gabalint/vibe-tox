//! The message log (REQ-INT-002, REQ-RPT-003, REQ-RPT-011). A row says what the
//! channel can prove: written to a folder or not, received or not, never "delivered".

use lis_core::{UtcTimestamp, text_enum};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::error::PartnerError;
use super::message_repo::NOT_REPLACED;
use super::model::{
    DeliveryReason, DeliveryState, MessageSource, PartnerConnectionId, PartnerDeliveryId,
    PartnerMessageId, PartnerMessageType, PartnerProtocol, UnroutedResolution,
};
use super::received::ReceivedState;
use super::received_queries::received_rows;

/// Rows the log returns at most, newest first.
const LOG_LIMIT: usize = 500;
/// Rows read before the view and search narrow them.
const SCAN_LIMIT: u32 = 2000;

text_enum! {
    /// Which log rows to show.
    #[ts(export)]
    MessageView {
        /// Every row.
        All = "all",
        /// Rows with a failed attempt.
        Failed = "failed",
        /// Rows not written yet, unrouted ones included.
        NotSent = "not_sent",
        /// Written rows, whose use the channel cannot confirm.
        OutcomeUnknown = "outcome_unknown",
    }
}

text_enum! {
    /// Did the message leave: what the transport can prove.
    #[ts(export)]
    TransportOutcome {
        /// The file is in the folder.
        Written = "written",
        /// An attempt failed; it is retried or waits to be sent again.
        Failed = "failed",
        /// Not written yet, or never: a newer revision replaced it.
        NotSent = "not_sent",
        /// An incoming message was read and stored.
        Received = "received",
    }
}

text_enum! {
    /// Why a message without a failed attempt is not written yet.
    #[ts(export)]
    WaitReason {
        /// The connection is stopped.
        Stopped = "stopped",
        /// No destination check covers the connection's folder.
        Unverified = "unverified",
        /// The connection writes at set times.
        Scheduled = "scheduled",
        /// It is written at the next pass.
        Queued = "queued",
        /// The report revision's impact review is open (L13). The engine, which
        /// knows reports, sets it.
        ImpactReview = "impact_review",
        /// A test or analyte of the HL7 message has no approved code mapping, so it
        /// is held, never sent with a guessed code (REQ-INT-004). Set by the engine.
        NotMapped = "not_mapped",
    }
}

/// Input of `list_partner_messages`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListPartnerMessages {
    /// Rows from this time on; all when absent.
    #[serde(default)]
    #[ts(optional)]
    pub since: Option<UtcTimestamp>,
    #[serde(default)]
    #[ts(optional)]
    pub connection_id: Option<PartnerConnectionId>,
    pub view: MessageView,
    /// A handoff number, a report or accession number, or part of a file name.
    #[serde(default)]
    #[ts(optional)]
    pub search: Option<String>,
}

/// A connection named in a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionRef {
    pub id: PartnerConnectionId,
    pub name: String,
    /// A file on an SFTP server shows as uploaded, in a folder as written.
    pub protocol: PartnerProtocol,
}

/// One message to one connection, or one unrouted message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerMessageRow {
    pub message_id: PartnerMessageId,
    pub delivery_id: Option<PartnerDeliveryId>,
    /// When it was written, last tried, or made.
    pub at: UtcTimestamp,
    pub message_type: PartnerMessageType,
    pub source: MessageSource,
    pub source_id: Option<String>,
    #[ts(type = "number | null")]
    pub source_number: Option<i64>,
    /// The report number, for a report's revision or withdrawal.
    pub source_label: Option<String>,
    pub connection: Option<ConnectionRef>,
    pub reason: Option<DeliveryReason>,
    pub transport: TransportOutcome,
    /// True when written: a folder never confirms that the partner used it.
    pub outcome_unknown: bool,
    pub waiting: Option<WaitReason>,
    /// Failed attempts since it was made or sent again.
    pub attempts: u32,
    pub given_up: bool,
    /// A newer revision or a withdrawal of the report replaced it before it was
    /// written; it is never written.
    pub superseded: bool,
    pub next_attempt_at: Option<UtcTimestamp>,
    pub error_code: Option<String>,
    pub written_path: Option<String>,
    /// The message matched no connection.
    pub unrouted: bool,
    /// For an unrouted message, the decision; `None` while it waits.
    pub resolution: Option<UnroutedResolution>,
    /// `resend_partner_delivery` once every attempt failed.
    pub allowed_actions: Vec<String>,
    /// For an incoming message: what it became. `None` for an outgoing one.
    pub received: Option<ReceivedState>,
}

/// The newest rows, and whether older ones were left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerMessagePage {
    pub rows: Vec<PartnerMessageRow>,
    pub more: bool,
}

/// The log, newest first, at most 500 rows.
pub fn list_partner_messages(
    conn: &Connection,
    input: &ListPartnerMessages,
) -> Result<PartnerMessagePage, PartnerError> {
    let mut rows = delivery_rows(conn, input)?;
    if input.connection_id.is_none() {
        rows.extend(unrouted_rows(conn, input.since)?);
    }
    rows.extend(received_rows(
        conn,
        input.since,
        input.connection_id,
        SCAN_LIMIT,
    )?);
    let search = input.search.as_deref().map(str::trim).unwrap_or_default();
    rows.retain(|row| in_view(row, input.view) && found(row, search));
    rows.sort_by_key(|row| std::cmp::Reverse(row.at));
    let more = rows.len() > LOG_LIMIT;
    rows.truncate(LOG_LIMIT);
    Ok(PartnerMessagePage { rows, more })
}

fn in_view(row: &PartnerMessageRow, view: MessageView) -> bool {
    match view {
        MessageView::All => true,
        MessageView::Failed => row.transport == TransportOutcome::Failed,
        MessageView::NotSent => row.transport == TransportOutcome::NotSent,
        MessageView::OutcomeUnknown => row.outcome_unknown,
    }
}

/// Digits find a handoff number; other text is found in the report number or
/// the written file's path.
fn found(row: &PartnerMessageRow, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }
    if search.bytes().all(|byte| byte.is_ascii_digit()) && row.source_label.is_none() {
        return row
            .source_number
            .is_some_and(|number| number.to_string() == search);
    }
    let search = search.to_lowercase();
    [row.source_label.as_deref(), row.written_path.as_deref()]
        .into_iter()
        .flatten()
        .any(|text| text.to_lowercase().contains(&search))
}

fn delivery_rows(
    conn: &Connection,
    input: &ListPartnerMessages,
) -> Result<Vec<PartnerMessageRow>, PartnerError> {
    conn.prepare_cached(
        "SELECT d.id, d.message_id, COALESCE(d.written_at, d.last_attempt_at, d.created_at) AS at, \
         m.message_type, m.source_kind, m.source_id, m.source_number, c.id, c.name, d.reason, \
         d.state, d.attempts, d.next_attempt_at, d.last_error_code, d.written_path, c.active, \
         EXISTS (SELECT 1 FROM ifc_partner_verifications v \
                 WHERE v.connection_id = c.id AND v.folder = c.destination), c.schedule, \
         m.source_label, c.protocol \
         FROM ifc_partner_deliveries d \
         JOIN ifc_partner_messages m ON m.id = d.message_id \
         JOIN ifc_partner_connections c ON c.id = d.connection_id \
         WHERE (?1 IS NULL OR COALESCE(d.written_at, d.last_attempt_at, d.created_at) >= ?1) \
         AND (?2 IS NULL OR c.id = ?2) \
         ORDER BY at DESC LIMIT ?3",
    )?
    .query_map(
        params![input.since, input.connection_id, SCAN_LIMIT],
        delivery_row,
    )?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

fn delivery_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PartnerMessageRow> {
    let state: DeliveryState = row.get(10)?;
    let attempts: u32 = row.get(11)?;
    let reason: DeliveryReason = row.get(9)?;
    // A synthetic test is never queued, so it is never sent again.
    let resendable = state == DeliveryState::Failed && reason != DeliveryReason::Test;
    let waiting = if state == DeliveryState::Waiting && attempts == 0 {
        Some(wait_reason(
            row.get(15)?,
            row.get(16)?,
            &row.get::<_, String>(17)?,
        ))
    } else {
        None
    };
    Ok(PartnerMessageRow {
        delivery_id: Some(row.get(0)?),
        message_id: row.get(1)?,
        at: row.get(2)?,
        message_type: row.get(3)?,
        source: row.get(4)?,
        source_id: row.get(5)?,
        source_number: row.get(6)?,
        source_label: row.get(18)?,
        connection: Some(ConnectionRef {
            id: row.get(7)?,
            name: row.get(8)?,
            protocol: row.get(19)?,
        }),
        reason: Some(reason),
        transport: transport(state, attempts),
        outcome_unknown: state == DeliveryState::Written,
        waiting,
        attempts,
        given_up: state == DeliveryState::Failed,
        superseded: state == DeliveryState::Superseded,
        next_attempt_at: row.get(12)?,
        error_code: row.get(13)?,
        written_path: row.get(14)?,
        unrouted: false,
        resolution: None,
        allowed_actions: if resendable {
            vec!["resend_partner_delivery".to_owned()]
        } else {
            Vec::new()
        },
        received: None,
    })
}

fn wait_reason(active: bool, verified: bool, schedule: &str) -> WaitReason {
    if !active {
        WaitReason::Stopped
    } else if !verified {
        WaitReason::Unverified
    } else if schedule == "immediately" {
        WaitReason::Queued
    } else {
        WaitReason::Scheduled
    }
}

fn transport(state: DeliveryState, attempts: u32) -> TransportOutcome {
    match state {
        DeliveryState::Written => TransportOutcome::Written,
        DeliveryState::Failed => TransportOutcome::Failed,
        DeliveryState::Waiting if attempts > 0 => TransportOutcome::Failed,
        DeliveryState::Waiting | DeliveryState::Superseded => TransportOutcome::NotSent,
    }
}

fn unrouted_rows(
    conn: &Connection,
    since: Option<UtcTimestamp>,
) -> Result<Vec<PartnerMessageRow>, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT m.id, m.created_at, m.message_type, m.source_kind, m.source_id, m.source_number, \
         r.resolution, m.source_label, NOT {NOT_REPLACED} FROM ifc_partner_messages m \
         LEFT JOIN ifc_partner_unrouted_resolutions r ON r.message_id = m.id \
         WHERE m.unrouted = 1 AND (?1 IS NULL OR m.created_at >= ?1) \
         ORDER BY m.created_at DESC LIMIT ?2"
    ))?
    .query_map(params![since, SCAN_LIMIT], |row| {
        Ok(PartnerMessageRow {
            message_id: row.get(0)?,
            delivery_id: None,
            at: row.get(1)?,
            message_type: row.get(2)?,
            source: row.get(3)?,
            source_id: row.get(4)?,
            source_number: row.get(5)?,
            source_label: row.get(7)?,
            connection: None,
            reason: None,
            transport: TransportOutcome::NotSent,
            outcome_unknown: false,
            waiting: None,
            attempts: 0,
            given_up: false,
            superseded: row.get(8)?,
            next_attempt_at: None,
            error_code: None,
            written_path: None,
            unrouted: true,
            resolution: row.get(6)?,
            allowed_actions: Vec::new(),
            received: None,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}
