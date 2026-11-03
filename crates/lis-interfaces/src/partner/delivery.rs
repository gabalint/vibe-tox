//! What the delivery poller may write now, and the outcome of each attempt
//! (REQ-RPT-003, REQ-RPT-011, REQ-RPT-012).
//!
//! A delivery is written only while its connection is started and a
//! destination check covers its current folder; every pass reads that again.
//! A failed attempt is retried after 1, 2, 4 and 8 minutes; after the fifth
//! the delivery fails and waits for a person to send it again. A written file
//! is "written", never "delivered": a folder gives no confirmation that the
//! partner took the file.

use std::time::Duration;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, params};

use super::error::PartnerError;
use super::message_repo::{self as repo, MessageRow};
use super::model::{
    DeliveryReason, DeliveryState, MessageSource, PartnerConnectionId, PartnerDeliveryId,
    PartnerMessageId, RoutingFacts, Schedule,
};
use super::repo::load_connection;

/// Attempts before a delivery fails for good.
pub const MAX_ATTEMPTS: i64 = 5;
/// Deliveries the poller takes per pass.
const BATCH: u32 = 50;

/// A delivery the poller may write now, subject to its schedule.
#[derive(Debug, Clone)]
pub struct DueDelivery {
    pub delivery_id: PartnerDeliveryId,
    pub message_id: PartnerMessageId,
    pub connection_id: PartnerConnectionId,
    pub folder: String,
    pub file_name_pattern: String,
    pub schedule: Schedule,
    /// Failed attempts so far; the schedule applies only to the first.
    pub attempts: i64,
    pub created_at: UtcTimestamp,
    pub artifact_sha256: String,
    pub source_number: Option<i64>,
}

/// How an attempt ended.
#[derive(Debug, Clone)]
pub enum AttemptOutcome {
    /// The file is in the folder, at `path`.
    Written { path: String },
    /// Nothing usable was written; `error_code` says why (PHI-free).
    Failed { error_code: &'static str },
}

/// Deliveries waiting on started, checked connections whose retry time has
/// come, oldest first. Synthetic test messages are never queued.
pub fn due_partner_deliveries(
    conn: &Connection,
    now: UtcTimestamp,
) -> Result<Vec<DueDelivery>, PartnerError> {
    let rows = conn
        .prepare_cached(
            "SELECT d.id, d.message_id, d.connection_id, d.attempts, d.created_at, \
             m.artifact_sha256, m.source_number \
             FROM ifc_partner_deliveries d \
             JOIN ifc_partner_messages m ON m.id = d.message_id \
             JOIN ifc_partner_connections c ON c.id = d.connection_id \
             WHERE d.state = 'waiting' AND d.reason <> 'test' AND c.active = 1 \
             AND (d.next_attempt_at IS NULL OR d.next_attempt_at <= ?1) \
             AND EXISTS (SELECT 1 FROM ifc_partner_verifications v \
                         WHERE v.connection_id = c.id AND v.folder = c.folder) \
             ORDER BY d.created_at LIMIT ?2",
        )?
        .query_map(params![now, BATCH], |row| {
            Ok((
                row.get::<_, PartnerDeliveryId>(0)?,
                row.get::<_, PartnerMessageId>(1)?,
                row.get::<_, PartnerConnectionId>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, UtcTimestamp>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut due = Vec::with_capacity(rows.len());
    for (delivery_id, message_id, connection_id, attempts, created_at, sha256, number) in rows {
        let connection =
            load_connection(conn, connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
        due.push(DueDelivery {
            delivery_id,
            message_id,
            connection_id,
            folder: connection.folder,
            file_name_pattern: connection.settings.file_name_pattern,
            schedule: connection.settings.schedule,
            attempts,
            created_at,
            artifact_sha256: sha256,
            source_number: number,
        });
    }
    Ok(due)
}

/// Records the poller's attempt on a waiting delivery, as the delivery
/// worker (`export`). Each attempt is a `security` event: a written file
/// discloses its content to the partner.
pub fn record_delivery_attempt(
    ctx: &mut CommandContext<'_>,
    delivery_id: PartnerDeliveryId,
    outcome: AttemptOutcome,
) -> Result<DeliveryState, PartnerError> {
    ctx.require(Capability::Export)?;
    let delivery =
        repo::load_delivery(ctx.tx(), delivery_id)?.ok_or(PartnerError::NotFound("delivery"))?;
    if delivery.state != DeliveryState::Waiting {
        return Err(PartnerError::WrongState("The delivery is not waiting."));
    }
    let now = ctx.now();
    let attempt = delivery.attempts + 1;
    let (state, audit_outcome, error_code) = match &outcome {
        AttemptOutcome::Written { path } => {
            repo::set_written(ctx.tx(), delivery_id, path, now)?;
            (DeliveryState::Written, Outcome::Succeeded, None)
        }
        AttemptOutcome::Failed { error_code } => {
            let retry_at = (attempt < MAX_ATTEMPTS)
                .then(|| retry_delay(attempt))
                .and_then(|delay| now.checked_add(delay));
            repo::set_failed_attempt(ctx.tx(), delivery_id, error_code, now, retry_at)?;
            let state = if retry_at.is_some() {
                DeliveryState::Waiting
            } else {
                DeliveryState::Failed
            };
            (state, Outcome::Failed, Some(*error_code))
        }
    };
    let mut event = AuditEvent::new(Stream::Security, "interfaces.write_message", audit_outcome)
        .record("partner_delivery", delivery_id)
        .detail("message_id", AuditDetail::id(delivery.message_id))
        .detail("connection_id", AuditDetail::id(delivery.connection_id))
        .detail("attempt", AuditDetail::integer(attempt))
        .detail("state", AuditDetail::code(state.as_str()));
    if let Some(code) = error_code {
        event = event.detail("error_code", AuditDetail::code(code));
    }
    ctx.audit(event)?;
    Ok(state)
}

/// 1, 2, 4 and 8 minutes after the first to fourth failure.
fn retry_delay(failures: i64) -> Duration {
    let exponent = u32::try_from(failures.clamp(1, 4) - 1).unwrap_or(0);
    Duration::from_secs(60 * 2_u64.pow(exponent))
}

/// Records a synthetic test message the engine wrote, or failed to write, to
/// a connection's folder. It carries no patient data, so it needs neither a
/// started connection nor a destination check, and is never retried.
pub fn record_partner_test_message(
    ctx: &mut CommandContext<'_>,
    connection_id: PartnerConnectionId,
    artifact_sha256: &str,
    outcome: &AttemptOutcome,
) -> Result<PartnerMessageId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let connection =
        load_connection(ctx.tx(), connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
    let message = MessageRow {
        id: PartnerMessageId::generate(),
        message_type: connection.settings.message_type,
        source: MessageSource::Test,
        source_id: None,
        source_number: None,
        artifact_sha256: artifact_sha256.to_owned(),
        facts: RoutingFacts::default(),
        checked_against: 1,
        unrouted: false,
        created_at: ctx.now(),
    };
    repo::insert_message(ctx.tx(), &message)?;
    let delivery = repo::insert_delivery(
        ctx.tx(),
        message.id,
        connection_id,
        DeliveryReason::Test,
        ctx.now(),
    )?;
    let (audit_outcome, written) = match outcome {
        AttemptOutcome::Written { path } => {
            repo::set_written(ctx.tx(), delivery, path, ctx.now())?;
            (Outcome::Succeeded, true)
        }
        AttemptOutcome::Failed { error_code } => {
            repo::set_failed_attempt(ctx.tx(), delivery, error_code, ctx.now(), None)?;
            (Outcome::Failed, false)
        }
    };
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interfaces.send_test_message",
            audit_outcome,
        )
        .record("partner_connection", connection_id)
        .detail("delivery_id", AuditDetail::id(delivery))
        .detail("written", AuditDetail::boolean(written)),
    )?;
    Ok(message.id)
}

/// A connection's current folder, for a test the engine runs on it.
pub fn partner_connection_folder(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<String, PartnerError> {
    Ok(load_connection(conn, connection_id)?
        .ok_or(PartnerError::NotFound("connection"))?
        .folder)
}
