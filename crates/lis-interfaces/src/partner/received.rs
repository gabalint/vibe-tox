//! Order messages read from an HL7 Orders connection (ticket #4555 step 3c; REQ-INT-002,
//! REQ-INT-003): each file read is kept with its original bytes, what became of it and
//! where the file moved. An exact replay is a duplicate with no effect; a message waiting
//! for its patient later becomes an order or is rejected. Recorded by the intake worker,
//! with `enter_orders`; a person deciding on a waiting message needs it too.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Id, UtcTimestamp, text_enum};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use super::error::PartnerError;
use super::model::PartnerConnectionId;

/// Marker for one message read from an HL7 Orders connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerReceivedMarker {}
/// One message read from an HL7 Orders connection.
pub type PartnerReceivedId = Id<PartnerReceivedMarker>;

text_enum! {
    /// What became of a received message.
    #[ts(export)]
    ReceivedState {
        /// It became an order in Received status, for the laboratory to accept or reject.
        OrderCreated = "order_created",
        /// It waits until its patient is registered with the identifier it carries.
        WaitingPatient = "waiting_patient",
        /// It waits for a person: the same message ID came with other content, or the
        /// patient's details differ from the registered patient's.
        Held = "held",
        /// The same bytes were read before; it had no effect.
        Duplicate = "duplicate",
        /// Refused; the file moved to Failed.
        Rejected = "rejected",
    }
}

/// What became of a message, as the intake worker decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceivedOutcome {
    OrderCreated { order_id: String },
    WaitingPatient { reason: &'static str },
    Held { reason: &'static str },
    Duplicate { of: PartnerReceivedId },
    Rejected { reason: &'static str },
}

impl ReceivedOutcome {
    const fn state(&self) -> ReceivedState {
        match self {
            Self::OrderCreated { .. } => ReceivedState::OrderCreated,
            Self::WaitingPatient { .. } => ReceivedState::WaitingPatient,
            Self::Held { .. } => ReceivedState::Held,
            Self::Duplicate { .. } => ReceivedState::Duplicate,
            Self::Rejected { .. } => ReceivedState::Rejected,
        }
    }

    const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::WaitingPatient { reason } | Self::Held { reason } | Self::Rejected { reason } => {
                Some(reason)
            }
            Self::OrderCreated { .. } | Self::Duplicate { .. } => None,
        }
    }

    /// Whether the file moves to Failed rather than Processed.
    #[must_use]
    pub const fn failed(&self) -> bool {
        matches!(self, Self::Rejected { .. })
    }
}

/// One message read.
#[derive(Debug, Clone)]
pub struct NewReceived {
    pub connection_id: PartnerConnectionId,
    pub file_name: String,
    pub artifact_sha256: String,
    pub control_id: Option<String>,
    pub placer_order: Option<String>,
    pub outcome: ReceivedOutcome,
}

/// An earlier read of the same bytes on the connection, and where its file moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlierRead {
    pub id: PartnerReceivedId,
    pub moved_to: Option<String>,
    /// Refused: its file belongs in Failed.
    pub rejected: bool,
}

/// The first read of these bytes on the connection, if any.
pub fn earlier_read(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    artifact_sha256: &str,
) -> Result<Option<EarlierRead>, PartnerError> {
    conn.prepare_cached(
        "SELECT id, moved_to, state = 'rejected' FROM ifc_partner_received \
         WHERE connection_id = ?1 AND artifact_sha256 = ?2 AND state <> 'duplicate' \
         ORDER BY received_at, id LIMIT 1",
    )?
    .query_row(params![connection_id, artifact_sha256], |row| {
        Ok(EarlierRead {
            id: row.get(0)?,
            moved_to: row.get(1)?,
            rejected: row.get(2)?,
        })
    })
    .optional()
    .map_err(Into::into)
}

/// Whether a message with this control ID (MSH-10) was read before on the connection.
pub fn control_id_seen(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    control_id: &str,
) -> Result<bool, PartnerError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM ifc_partner_received \
         WHERE connection_id = ?1 AND control_id = ?2)",
        params![connection_id, control_id],
        |row| row.get(0),
    )?)
}

/// Records one message read, in the transaction that made its order, if any.
pub fn record_received(
    ctx: &mut CommandContext<'_>,
    new: NewReceived,
) -> Result<PartnerReceivedId, PartnerError> {
    ctx.require(Capability::EnterOrders)?;
    let id = PartnerReceivedId::generate();
    let state = new.outcome.state();
    let (order_id, duplicate_of) = match &new.outcome {
        ReceivedOutcome::OrderCreated { order_id } => (Some(order_id.clone()), None),
        ReceivedOutcome::Duplicate { of } => (None, Some(*of)),
        ReceivedOutcome::WaitingPatient { .. }
        | ReceivedOutcome::Held { .. }
        | ReceivedOutcome::Rejected { .. } => (None, None),
    };
    ctx.tx().execute(
        "INSERT INTO ifc_partner_received (id, connection_id, file_name, artifact_sha256, \
         received_at, control_id, placer_order, state, reason_code, order_id, duplicate_of) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            new.connection_id,
            new.file_name,
            new.artifact_sha256,
            ctx.now(),
            new.control_id,
            new.placer_order,
            state,
            new.outcome.reason(),
            order_id,
            duplicate_of
        ],
    )?;
    audit(
        ctx,
        "interfaces.receive_order",
        id,
        new.connection_id,
        &new.outcome,
    )?;
    Ok(id)
}

/// Records where the file moved: Processed or Failed, by year and month.
pub fn record_moved(
    ctx: &mut CommandContext<'_>,
    id: PartnerReceivedId,
    path: &str,
) -> Result<(), PartnerError> {
    ctx.require(Capability::EnterOrders)?;
    ctx.tx().execute(
        "UPDATE ifc_partner_received SET moved_to = ?2 WHERE id = ?1 AND moved_to IS NULL",
        params![id, path],
    )?;
    Ok(())
}

/// A message waiting for its patient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingMessage {
    pub id: PartnerReceivedId,
    pub connection_id: PartnerConnectionId,
    pub artifact_sha256: String,
    pub received_at: UtcTimestamp,
}

/// Every message waiting for its patient, oldest first.
pub fn waiting_messages(conn: &Connection) -> Result<Vec<WaitingMessage>, PartnerError> {
    conn.prepare_cached(
        "SELECT id, connection_id, artifact_sha256, received_at FROM ifc_partner_received \
         WHERE state = 'waiting_patient' ORDER BY received_at, id",
    )?
    .query_map([], |row| {
        Ok(WaitingMessage {
            id: row.get(0)?,
            connection_id: row.get(1)?,
            artifact_sha256: row.get(2)?,
            received_at: row.get(3)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// Ends a waiting message: its order was made once the patient was registered, the
/// registered patient's details differ so a person decides, or it is rejected. The intake
/// worker or a person decides.
pub fn resolve_waiting(
    ctx: &mut CommandContext<'_>,
    id: PartnerReceivedId,
    outcome: ReceivedOutcome,
) -> Result<(), PartnerError> {
    ctx.require(Capability::EnterOrders)?;
    let (connection_id, state): (PartnerConnectionId, ReceivedState) = ctx
        .tx()
        .query_row(
            "SELECT connection_id, state FROM ifc_partner_received WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(PartnerError::NotFound("received message"))?;
    let order_id = match (&outcome, state) {
        (_, state) if state != ReceivedState::WaitingPatient => {
            return Err(PartnerError::WrongState(
                "The message is no longer waiting.",
            ));
        }
        (ReceivedOutcome::OrderCreated { order_id }, _) => Some(order_id.clone()),
        (ReceivedOutcome::Rejected { .. } | ReceivedOutcome::Held { .. }, _) => None,
        _ => {
            return Err(PartnerError::WrongState(
                "A waiting message becomes an order, is held or is rejected.",
            ));
        }
    };
    let decided_by = ctx.principal().actor().id_text();
    ctx.tx().execute(
        "UPDATE ifc_partner_received SET state = ?2, order_id = ?3, \
         reason_code = COALESCE(?4, reason_code), decided_by = ?5, decided_at = ?6 WHERE id = ?1",
        params![
            id,
            outcome.state(),
            order_id,
            outcome.reason(),
            decided_by,
            ctx.now()
        ],
    )?;
    audit(
        ctx,
        "interfaces.resolve_received",
        id,
        connection_id,
        &outcome,
    )
}

fn audit(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    id: PartnerReceivedId,
    connection_id: PartnerConnectionId,
    outcome: &ReceivedOutcome,
) -> Result<(), PartnerError> {
    let result = if outcome.failed() {
        Outcome::Failed
    } else {
        Outcome::Succeeded
    };
    let mut event = AuditEvent::new(Stream::Clinical, action, result)
        .record("partner_received", id)
        .detail("connection_id", AuditDetail::id(connection_id))
        .detail("state", AuditDetail::code(outcome.state().as_str()));
    if let Some(reason) = outcome.reason() {
        event = event.detail("reason", AuditDetail::code(reason));
    }
    ctx.audit(event)?;
    Ok(())
}
