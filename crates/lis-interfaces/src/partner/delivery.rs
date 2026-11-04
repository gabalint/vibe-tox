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
    DeliveryReason, DeliveryState, MessageSource, MovedTo, PartnerConnectionId, PartnerDeliveryId,
    PartnerMessageId, PartnerMessageType, RoutingFacts, Schedule, SftpSettings,
};
use super::repo::{ConnectionRow, load_connection};
use super::settings::{FileNameParts, file_name};

/// Attempts before a delivery fails for good.
pub const MAX_ATTEMPTS: i64 = 5;
/// Deliveries the poller takes per pass.
const BATCH: u32 = 50;

/// Where a connection writes: its local folder, or its SFTP server and remote folder with
/// the sealed password for password sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    /// The local folder, or the remote folder: written to, or for incoming orders read.
    pub folder: String,
    pub sftp: Option<SftpSettings>,
    pub sealed_password: Option<Vec<u8>>,
    /// For incoming orders: where read files move to.
    pub moved_to: Option<MovedTo>,
}

impl Destination {
    pub(crate) fn of(connection: &ConnectionRow) -> Self {
        Self {
            folder: connection.folder.clone(),
            sftp: connection.settings.sftp.clone(),
            sealed_password: connection.sealed_password.clone(),
            moved_to: connection.moved_to.clone(),
        }
    }
}

/// A delivery the poller may write now, subject to its schedule.
#[derive(Debug, Clone)]
pub struct DueDelivery {
    pub delivery_id: PartnerDeliveryId,
    pub message_id: PartnerMessageId,
    pub connection_id: PartnerConnectionId,
    pub destination: Destination,
    pub file_name_pattern: String,
    pub schedule: Schedule,
    /// Failed attempts so far; the schedule applies only to the first.
    pub attempts: i64,
    pub created_at: UtcTimestamp,
    pub message_type: PartnerMessageType,
    /// The source's stored file: the handoff file, report PDF or withdrawal notice.
    pub artifact_sha256: String,
    /// The bytes an HL7 delivery made at its first attempt; every retry writes them.
    pub payload_sha256: Option<String>,
    /// The partner the connection sends to (HL7 MSH-6).
    pub partner: String,
    pub source: MessageSource,
    /// The report revision, for a report's revision.
    pub source_id: Option<String>,
    /// The handoff or revision number.
    pub source_number: Option<i64>,
    /// The report number, for a report's revision or withdrawal.
    pub source_label: Option<String>,
}

impl DueDelivery {
    /// The file name the connection's pattern makes for this message, with `date`
    /// (`YYYYMMDD`) and `time` (`HHMMSS`). A withdrawal notice's revision reads
    /// `<n>-withdrawn`, so it never takes the name of the revision it withdraws.
    #[must_use]
    pub fn file_name(&self, date: &str, time: &str) -> String {
        let number = self
            .source_number
            .map_or_else(|| self.delivery_id.to_string(), |n| n.to_string());
        let revision = if self.source == MessageSource::ReportWithdrawal {
            format!("{number}-withdrawn")
        } else {
            number.clone()
        };
        let record = self.source_label.clone().unwrap_or(number);
        file_name(
            &self.file_name_pattern,
            &FileNameParts {
                record: &record,
                revision: &revision,
                date,
                time,
            },
        )
    }
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
/// come: a report's later revisions and withdrawal notices first, then oldest
/// first. Synthetic test messages are never queued.
pub fn due_partner_deliveries(
    conn: &Connection,
    now: UtcTimestamp,
) -> Result<Vec<DueDelivery>, PartnerError> {
    let rows = conn
        .prepare_cached(
            "SELECT d.id, d.message_id, d.connection_id, d.attempts, d.created_at, \
             m.artifact_sha256, m.source_kind, m.source_id, m.source_number, m.source_label, \
             m.message_type, d.payload_sha256 \
             FROM ifc_partner_deliveries d \
             JOIN ifc_partner_messages m ON m.id = d.message_id \
             JOIN ifc_partner_connections c ON c.id = d.connection_id \
             WHERE d.state = 'waiting' AND d.reason <> 'test' AND c.active = 1 \
             AND (d.next_attempt_at IS NULL OR d.next_attempt_at <= ?1) \
             AND EXISTS (SELECT 1 FROM ifc_partner_verifications v \
                         WHERE v.connection_id = c.id AND v.folder = c.destination) \
             ORDER BY d.reason = 'follow_up' DESC, d.created_at LIMIT ?2",
        )?
        .query_map(params![now, BATCH], |row| {
            Ok(DueDelivery {
                delivery_id: row.get(0)?,
                message_id: row.get(1)?,
                connection_id: row.get(2)?,
                destination: Destination {
                    folder: String::new(),
                    sftp: None,
                    sealed_password: None,
                    moved_to: None,
                },
                file_name_pattern: String::new(),
                schedule: Schedule::Immediately,
                attempts: row.get(3)?,
                created_at: row.get(4)?,
                message_type: row.get(10)?,
                artifact_sha256: row.get(5)?,
                payload_sha256: row.get(11)?,
                partner: String::new(),
                source: row.get(6)?,
                source_id: row.get(7)?,
                source_number: row.get(8)?,
                source_label: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|due| {
            let connection = load_connection(conn, due.connection_id)?
                .ok_or(PartnerError::NotFound("connection"))?;
            Ok(DueDelivery {
                destination: Destination::of(&connection),
                file_name_pattern: connection.settings.file_name_pattern,
                schedule: connection.settings.schedule,
                partner: connection.settings.partner,
                ..due
            })
        })
        .collect()
}

/// Records the bytes an HL7 delivery made at its first attempt, and the code mapping
/// version they used, before they are written; every retry then writes the same bytes.
pub fn record_delivery_payload(
    ctx: &mut CommandContext<'_>,
    delivery_id: PartnerDeliveryId,
    payload_sha256: &str,
    mapping_version: i64,
) -> Result<(), PartnerError> {
    ctx.require(Capability::Export)?;
    if repo::set_payload(ctx.tx(), delivery_id, payload_sha256, mapping_version)? == 0 {
        return Err(PartnerError::WrongState(
            "The delivery is not waiting, or already has its file.",
        ));
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interfaces.make_message",
            Outcome::Succeeded,
        )
        .record("partner_delivery", delivery_id)
        .detail("mapping_version", AuditDetail::integer(mapping_version)),
    )?;
    Ok(())
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
        source_group: None,
        source_label: None,
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

/// Where and what a connection writes, for a test the engine runs on it.
#[derive(Debug, Clone)]
pub struct ConnectionTarget {
    pub destination: Destination,
    pub message_type: PartnerMessageType,
    pub partner: String,
}

/// A connection's current destination, message type and partner.
pub fn partner_connection_folder(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<ConnectionTarget, PartnerError> {
    let connection =
        load_connection(conn, connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
    Ok(ConnectionTarget {
        destination: Destination::of(&connection),
        message_type: connection.settings.message_type,
        partner: connection.settings.partner,
    })
}
