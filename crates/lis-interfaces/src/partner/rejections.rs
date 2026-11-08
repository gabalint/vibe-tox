//! A person's follow-up of rejected messages (ticket #4555 step 5c; REQ-INT-002). A message
//! the LIS refused, or one the partner rejected with AE or AR, stays under Rejected in Logs
//! until a person resolves it with a note. A message waiting for its patient or held for
//! review can be rejected by a person with a reason, which also resolves it: no order is
//! made, and the partner is not told; over MLLP it got AA when it arrived.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::error::{PartnerError, invalid};
use super::model::{AckCode, PartnerConnectionId, PartnerDeliveryId};
use super::received::{PartnerReceivedId, ReceivedState};
use crate::model::ChangeStamp;

/// The longest reason or note.
const NOTE_MAX: usize = 500;
/// The reason code of a message a person rejected.
pub(crate) const REJECTED_BY_PERSON: &str = "rejected_by_person";
/// The columns [`resolution_at`] reads, from `x`, the resolutions table.
pub(crate) const RESOLUTION_COLUMNS: &str = "x.note, x.resolved_by, x.resolved_at";

/// Input of [`reject_received_message`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RejectReceivedMessage {
    pub received_id: PartnerReceivedId,
    /// Why, in the person's words.
    pub reason: String,
}

/// Input of [`resolve_partner_rejection`]: a received message or a sent one, not both.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolvePartnerRejection {
    #[serde(default)]
    #[ts(optional)]
    pub received_id: Option<PartnerReceivedId>,
    #[serde(default)]
    #[ts(optional)]
    pub delivery_id: Option<PartnerDeliveryId>,
    /// What was done about it, such as the clinic sending the order again.
    pub note: String,
}

/// A resolved rejection: the note, who resolved it and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RejectionResolution {
    pub note: String,
    pub resolved: ChangeStamp,
}

/// Rejects a received message that waits for its patient or is held for review; the reason
/// resolves it. Returns its connection.
pub fn reject_received_message(
    ctx: &mut CommandContext<'_>,
    input: RejectReceivedMessage,
) -> Result<PartnerConnectionId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let reason = text("reason", &input.reason)?;
    let id = input.received_id;
    let (connection_id, state) = received(ctx.tx(), id)?;
    if !matches!(state, ReceivedState::WaitingPatient | ReceivedState::Held) {
        return Err(PartnerError::WrongState(
            "Only a message waiting for its patient or held for review can be rejected.",
        ));
    }
    let (by, at) = (ctx.principal().actor().id_text(), ctx.now());
    ctx.tx().execute(
        "UPDATE ifc_partner_received SET state = ?2, reason_code = ?3, decided_by = ?4, \
         decided_at = ?5 WHERE id = ?1",
        params![id, ReceivedState::Rejected, REJECTED_BY_PERSON, by, at],
    )?;
    insert(ctx.tx(), (Some(id), None), &reason, &by, at)?;
    let event = AuditEvent::new(
        Stream::Clinical,
        "interfaces.reject_received",
        Outcome::Succeeded,
    )
    .record("partner_received", id)
    .detail("connection_id", AuditDetail::id(connection_id))
    .detail("state", AuditDetail::code(state.as_str()))
    .reason(reason);
    ctx.audit(event)?;
    Ok(connection_id)
}

/// Resolves a rejection with a note: a received message the LIS refused, or a sent one the
/// partner rejected with AE or AR. Once only.
pub fn resolve_partner_rejection(
    ctx: &mut CommandContext<'_>,
    input: ResolvePartnerRejection,
) -> Result<(), PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let note = text("note", &input.note)?;
    let event = AuditEvent::new(
        Stream::Clinical,
        "interfaces.resolve_rejection",
        Outcome::Succeeded,
    );
    let event = match (input.received_id, input.delivery_id) {
        (Some(id), None) => {
            if received(ctx.tx(), id)?.1 != ReceivedState::Rejected {
                return Err(PartnerError::WrongState("This message was not rejected."));
            }
            event.record("partner_received", id)
        }
        (None, Some(id)) => {
            if !matches!(
                delivery_ack(ctx.tx(), id)?,
                Some(AckCode::Error | AckCode::Rejected)
            ) {
                return Err(PartnerError::WrongState(
                    "The partner did not reject this message.",
                ));
            }
            event.record("partner_delivery", id)
        }
        _ => {
            return Err(invalid(
                "receivedId",
                "one_message",
                "Choose one received or one sent message.",
            ));
        }
    };
    let (by, at) = (ctx.principal().actor().id_text(), ctx.now());
    insert(
        ctx.tx(),
        (input.received_id, input.delivery_id),
        &note,
        &by,
        at,
    )?;
    ctx.audit(event.reason(note))?;
    Ok(())
}

/// The resolution read from [`RESOLUTION_COLUMNS`] starting at column `first`.
pub(crate) fn resolution_at(
    row: &rusqlite::Row<'_>,
    first: usize,
) -> rusqlite::Result<Option<RejectionResolution>> {
    let note: Option<String> = row.get(first)?;
    let Some(note) = note else {
        return Ok(None);
    };
    Ok(Some(RejectionResolution {
        note,
        resolved: ChangeStamp {
            by: row.get(first + 1)?,
            by_display: None,
            at: row.get(first + 2)?,
        },
    }))
}

fn text(field: &str, value: &str) -> Result<String, PartnerError> {
    required_text(field, value, NOTE_MAX).map_err(PartnerError::Invalid)
}

fn received(
    conn: &Connection,
    id: PartnerReceivedId,
) -> Result<(PartnerConnectionId, ReceivedState), PartnerError> {
    conn.query_row(
        "SELECT connection_id, state FROM ifc_partner_received WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?
    .ok_or(PartnerError::NotFound("received message"))
}

fn delivery_ack(conn: &Connection, id: PartnerDeliveryId) -> Result<Option<AckCode>, PartnerError> {
    conn.query_row(
        "SELECT ack_code FROM ifc_partner_deliveries WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(PartnerError::NotFound("delivery"))
}

/// Records the resolution; a rejection resolved before is refused.
fn insert(
    conn: &Connection,
    (received_id, delivery_id): (Option<PartnerReceivedId>, Option<PartnerDeliveryId>),
    note: &str,
    by: &str,
    at: UtcTimestamp,
) -> Result<(), PartnerError> {
    let resolved: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM ifc_partner_rejection_resolutions \
         WHERE received_id IS ?1 AND delivery_id IS ?2)",
        params![received_id, delivery_id],
        |row| row.get(0),
    )?;
    if resolved {
        return Err(PartnerError::WrongState(
            "This rejection is already resolved.",
        ));
    }
    conn.execute(
        "INSERT INTO ifc_partner_rejection_resolutions \
         (received_id, delivery_id, note, resolved_by, resolved_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![received_id, delivery_id, note, by, at],
    )?;
    Ok(())
}
