//! Routing a new message, deciding on unrouted ones and sending a failed one
//! again (REQ-RPT-003, REQ-INT-003).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, required_text};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::error::{PartnerError, invalid};
use super::message_repo::{self as repo, MessageRow, ResolutionRow};
use super::model::{
    DeliveryReason, DeliveryState, MessageSource, NewPartnerMessage, PartnerConnectionId,
    PartnerDeliveryId, PartnerMessageId, Routed, UnroutedResolution,
};
use super::repo::connections;
use super::rules::rules_match;

const NOTE_MAX: usize = 500;

/// What to do with a message that matched no connection.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum UnroutedDecision {
    /// Send it once to `connection`, which must carry its type.
    SendOnce { connection: PartnerConnectionId },
    /// Route it by the connections' rules as they are now.
    RetryRouting,
    /// Record that it was delivered outside the LIS, and how.
    DeliverAnotherWay { method: String },
    /// Leave it undelivered, for `reason`.
    Dismiss { reason: String },
}

/// Input of `resolve_unrouted_message`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolveUnroutedMessage {
    pub message_id: PartnerMessageId,
    pub decision: UnroutedDecision,
}

/// Input of `resend_partner_delivery`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResendPartnerDelivery {
    pub delivery_id: PartnerDeliveryId,
}

/// Routes `message` in the transaction that creates its source: one delivery
/// to every connection of its type whose rules match, or, when none match, a
/// place in Unrouted messages. Without any connection of the type nothing is
/// kept, so a laboratory that exports by hand gets no unrouted messages. The
/// caller has already authorized creating the source.
///
/// A report's later revision goes first to every connection that had an earlier
/// one, whatever its rules (REQ-RPT-013), and earlier revisions not written yet
/// are superseded. A withdrawal notice goes only to connections an earlier
/// revision was written to; with none, nothing is kept.
pub fn route_partner_message(
    ctx: &mut CommandContext<'_>,
    message: NewPartnerMessage,
) -> Result<Routed, PartnerError> {
    let candidates = connections(ctx.tx(), Some(message.message_type))?;
    if candidates.is_empty() {
        return Ok(Routed::NoConnection);
    }
    let withdrawal = message.source == MessageSource::ReportWithdrawal;
    let earlier = match &message.report {
        Some(report) => repo::report_recipients(ctx.tx(), message.message_type, &report.report_id)?,
        None => Vec::new(),
    };
    let follow_ups: Vec<PartnerConnectionId> = earlier
        .iter()
        .filter(|(_, written)| *written || !withdrawal)
        .map(|(connection, _)| *connection)
        .collect();
    if withdrawal && follow_ups.is_empty() {
        return Ok(Routed::NoConnection);
    }
    let matched: Vec<PartnerConnectionId> = candidates
        .iter()
        .filter(|connection| {
            !withdrawal
                && !follow_ups.contains(&connection.id)
                && rules_match(&connection.settings.rules, &message.facts)
        })
        .map(|connection| connection.id)
        .collect();
    let superseded = match &message.report {
        Some(report) => {
            repo::supersede_unwritten(ctx.tx(), message.message_type, &report.report_id)?
        }
        None => 0,
    };
    let row = new_row(
        ctx,
        message,
        candidates.len(),
        follow_ups.is_empty() && matched.is_empty(),
    )?;
    repo::insert_message(ctx.tx(), &row)?;
    let deliveries = follow_ups
        .iter()
        .map(|id| (*id, DeliveryReason::FollowUp))
        .chain(matched.iter().map(|id| (*id, DeliveryReason::Routed)));
    for (connection, reason) in deliveries {
        repo::insert_delivery(ctx.tx(), row.id, connection, reason, ctx.now())?;
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "interfaces.route_message",
            Outcome::Succeeded,
        )
        .record("partner_message", row.id)
        .detail("source", AuditDetail::code(row.source.as_str()))
        .detail("deliveries", AuditDetail::count(matched.len()))
        .detail("follow_ups", AuditDetail::count(follow_ups.len()))
        .detail("superseded", AuditDetail::count(superseded))
        .detail("unrouted", AuditDetail::boolean(row.unrouted)),
    )?;
    Ok(Routed::Kept {
        message_id: row.id,
        deliveries: follow_ups.len() + matched.len(),
    })
}

fn new_row(
    ctx: &CommandContext<'_>,
    message: NewPartnerMessage,
    checked_against: usize,
    unrouted: bool,
) -> Result<MessageRow, PartnerError> {
    let (source_group, source_label) = message
        .report
        .map(|report| (report.report_id, report.report_number))
        .unzip();
    Ok(MessageRow {
        id: PartnerMessageId::generate(),
        message_type: message.message_type,
        source: message.source,
        source_id: Some(message.source_id),
        source_number: Some(message.source_number),
        source_group,
        source_label,
        artifact_sha256: message.artifact_sha256,
        facts: message.facts,
        checked_against: i64::try_from(checked_against)
            .map_err(|_| PartnerError::Inconsistent("connection count"))?,
        unrouted,
        created_at: ctx.now(),
    })
}

/// Records the decision on an unrouted message. Every decision is final and
/// audited; sending and routing again create deliveries the poller writes
/// under the usual checks.
pub fn resolve_unrouted_message(
    ctx: &mut CommandContext<'_>,
    input: ResolveUnroutedMessage,
) -> Result<PartnerMessageId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let message =
        repo::load_message(ctx.tx(), input.message_id)?.ok_or(PartnerError::NotFound("message"))?;
    if !message.unrouted
        || repo::resolution(ctx.tx(), message.id)?.is_some()
        || repo::replaced(ctx.tx(), message.id)?
    {
        return Err(PartnerError::WrongState(
            "This message is no longer waiting in Unrouted messages.",
        ));
    }
    let (resolution, connection_id, note, deliveries) = decide(ctx, &message, input.decision)?;
    repo::insert_resolution(
        ctx.tx(),
        message.id,
        &ResolutionRow {
            resolution,
            connection_id,
            note: note.clone(),
            resolved_by: ctx.principal().actor().id_text(),
            resolved_at: ctx.now(),
        },
    )?;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        "interfaces.resolve_unrouted",
        Outcome::Succeeded,
    )
    .record("partner_message", message.id)
    .detail("resolution", AuditDetail::code(resolution.as_str()))
    .detail("deliveries", AuditDetail::count(deliveries));
    if let Some(note) = note {
        event = event.reason(note);
    }
    ctx.audit(event)?;
    Ok(message.id)
}

type Decided = (
    UnroutedResolution,
    Option<PartnerConnectionId>,
    Option<String>,
    usize,
);

fn decide(
    ctx: &CommandContext<'_>,
    message: &MessageRow,
    decision: UnroutedDecision,
) -> Result<Decided, PartnerError> {
    let deliver = |connection| {
        repo::insert_delivery(
            ctx.tx(),
            message.id,
            connection,
            DeliveryReason::SentOnce,
            ctx.now(),
        )
    };
    Ok(match decision {
        UnroutedDecision::SendOnce { connection } => {
            let carries = connections(ctx.tx(), Some(message.message_type))?
                .iter()
                .any(|candidate| candidate.id == connection);
            if !carries {
                return Err(invalid(
                    "decision.connection",
                    "wrong_type",
                    "Choose a connection that carries this message type.",
                ));
            }
            deliver(connection)?;
            (UnroutedResolution::SentOnce, Some(connection), None, 1)
        }
        UnroutedDecision::RetryRouting => {
            let matched: Vec<_> = connections(ctx.tx(), Some(message.message_type))?
                .into_iter()
                .filter(|connection| rules_match(&connection.settings.rules, &message.facts))
                .collect();
            if matched.is_empty() {
                return Err(PartnerError::WrongState(
                    "No connection's rules match this message yet. Change a rule first.",
                ));
            }
            for connection in &matched {
                repo::insert_delivery(
                    ctx.tx(),
                    message.id,
                    connection.id,
                    DeliveryReason::Routed,
                    ctx.now(),
                )?;
            }
            (UnroutedResolution::RoutedAgain, None, None, matched.len())
        }
        UnroutedDecision::DeliverAnotherWay { method } => {
            let method = note("decision.method", &method)?;
            (
                UnroutedResolution::DeliveredAnotherWay,
                None,
                Some(method),
                0,
            )
        }
        UnroutedDecision::Dismiss { reason } => {
            let reason = note("decision.reason", &reason)?;
            (UnroutedResolution::Dismissed, None, Some(reason), 0)
        }
    })
}

fn note(field: &str, text: &str) -> Result<String, PartnerError> {
    required_text(field, text, NOTE_MAX).map_err(PartnerError::Invalid)
}

/// Sends a failed delivery again: the same message to the same connection,
/// with a fresh set of attempts. It never makes a new message or report.
pub fn resend_partner_delivery(
    ctx: &mut CommandContext<'_>,
    input: ResendPartnerDelivery,
) -> Result<PartnerDeliveryId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let delivery = repo::load_delivery(ctx.tx(), input.delivery_id)?
        .ok_or(PartnerError::NotFound("delivery"))?;
    if delivery.state != DeliveryState::Failed {
        return Err(PartnerError::WrongState(
            "Only a message whose attempts all failed can be sent again.",
        ));
    }
    if delivery.reason == DeliveryReason::Test {
        return Err(PartnerError::WrongState(
            "A test message is not sent again. Send a new test message.",
        ));
    }
    repo::set_waiting_again(ctx.tx(), delivery.id)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "interfaces.resend_delivery",
            Outcome::Succeeded,
        )
        .record("partner_delivery", delivery.id)
        .detail("message_id", AuditDetail::id(delivery.message_id))
        .detail("connection_id", AuditDetail::id(delivery.connection_id))
        .detail("attempts", AuditDetail::integer(delivery.attempts)),
    )?;
    Ok(delivery.id)
}
