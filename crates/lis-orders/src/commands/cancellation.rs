//! Cancellation requests and decisions preserve prior work and supplied evidence.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};

use crate::error::OrderError;
use crate::inputs::{DecideCancellationInput, RequestCancellationInput};
use crate::queries::get_order;
use crate::{CancellationRequestId, Decision, ItemState, OrderId, OrderItemId};

use super::validation::{reason, required};

/// Records a cancellation request without changing or erasing existing work.
pub fn request_cancellation(
    ctx: &mut CommandContext<'_>,
    input: RequestCancellationInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    required("requestedBy", &input.requested_by)?;
    reason("reason", &input.reason)?;
    let (order_id, state): (OrderId, ItemState) = ctx
        .tx()
        .query_row(
            "SELECT order_id, state FROM ord_order_items WHERE id = ?1",
            [input.item_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order_item"))?;
    if state != ItemState::Accepted {
        return Err(OrderError::InvalidTransition);
    }
    let existing: Option<(String, String)> = ctx
        .tx()
        .query_row(
            "SELECT requested_by_text, reason FROM ord_cancellation_requests \
         WHERE item_id = ?1 AND state = 'requested'",
            [input.item_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((requested_by, reason)) = existing {
        return if requested_by == input.requested_by && reason == input.reason {
            get_order(ctx.tx(), order_id)
        } else {
            Err(OrderError::ReplayConflict)
        };
    }
    let id = CancellationRequestId::generate();
    ctx.tx().execute(
        "INSERT INTO ord_cancellation_requests (id, item_id, requested_by_text, reason, state, revision) \
         VALUES (?1, ?2, ?3, ?4, 'requested', 1)",
        params![id, input.item_id, input.requested_by, input.reason],
    )?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.request_cancel", Outcome::Succeeded)
            .record("cancellation_request", id)
            .new_revision(1)
            .reason(input.reason)
            .detail("order_id", AuditDetail::id(order_id))
            .detail("item_id", AuditDetail::id(input.item_id)),
    )?;
    bump_order(ctx, order_id)?;
    get_order(ctx.tx(), order_id)
}

/// Accepts or declines a cancellation. Acceptance cancels only the item and
/// its components; executions, observations and supplied evidence remain.
pub fn decide_cancellation(
    ctx: &mut CommandContext<'_>,
    input: DecideCancellationInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    reason("reason", &input.reason)?;
    let (item_id, order_id, state, revision): (OrderItemId, OrderId, String, i64) = ctx
        .tx()
        .query_row(
            "SELECT c.item_id, i.order_id, c.state, c.revision FROM ord_cancellation_requests c \
             JOIN ord_order_items i ON i.id = c.item_id WHERE c.id = ?1",
            [input.request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("cancellation_request"))?;
    if revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if state != "requested" {
        return Err(OrderError::InvalidTransition);
    }
    let next_state = match input.decision {
        Decision::Accept => "accepted",
        Decision::Decline => "declined",
    };
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.decide_cancel", Outcome::Succeeded)
            .record("cancellation_request", input.request_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(input.reason.clone())
            .detail("decision", AuditDetail::code(input.decision.as_str())),
    )?;
    ctx.tx().execute(
        "UPDATE ord_cancellation_requests SET state = ?2, decided_by = ?3, decided_at = ?4, \
         decision_reason = ?5, revision = revision + 1 WHERE id = ?1",
        params![
            input.request_id,
            next_state,
            ctx.principal().actor().id_text(),
            ctx.now(),
            input.reason
        ],
    )?;
    if input.decision == Decision::Accept {
        ctx.tx().execute(
            "UPDATE ord_order_items SET state = 'cancelled', revision = revision + 1 WHERE id = ?1",
            [item_id],
        )?;
        ctx.tx().execute(
            "UPDATE ord_order_components SET state = 'cancelled', revision = revision + 1 \
             WHERE item_id = ?1 AND state = 'active'",
            [item_id],
        )?;
    }
    bump_order(ctx, order_id)?;
    get_order(ctx.tx(), order_id)
}

fn bump_order(ctx: &CommandContext<'_>, order_id: OrderId) -> Result<(), OrderError> {
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = revision + 1 WHERE id = ?1",
        [order_id],
    )?;
    Ok(())
}
