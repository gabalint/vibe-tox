//! Add-on/reflex requests, explicit trigger decisions and cancellation requests.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};

use crate::error::{OrderError, invalid};
use crate::inputs::{DecideFollowOnInput, LaboratoryResultFollowOnInput, RequestFollowOnInput};
use crate::queries::get_order;
use crate::{Decision, ItemKind, ItemState, OrderId, OrderItemId, OrderStatus};

use super::order_support::{bump_order, freeze_components, load_order_state};
use super::trigger::{
    TriggerBoundary, ensure_parent_order, ensure_reflex_scope, insert_activation,
    replayed_activation, require_at_boundary, validate_follow_on_trigger,
};
use super::validation::{active_orderable, optional, reason};

/// Requests an add-on or an authorized in-scope reflex (REQ-ORD-002/003).
pub fn request_follow_on(
    ctx: &mut CommandContext<'_>,
    input: RequestFollowOnInput,
) -> Result<crate::OrderDetail, OrderError> {
    request_follow_on_at_boundary(ctx, input, TriggerBoundary::Public)
}

/// Requests a reflex from a laboratory result revision verified by the results owner.
///
/// Before calling, the results owner must authorize the caller and verify the
/// current result revision and its order in this same transaction.
pub fn request_follow_on_from_result(
    ctx: &mut CommandContext<'_>,
    input: LaboratoryResultFollowOnInput,
) -> Result<crate::OrderDetail, OrderError> {
    let request = RequestFollowOnInput {
        order_id: input.order_id,
        expected_revision: input.expected_revision,
        kind: ItemKind::Reflex,
        orderable_version_id: input.orderable_version_id,
        sequence_role: input.sequence_role,
        parent_item_id: Some(input.parent_item_id),
        authority_reference: None,
        poc_result_id: None,
        trigger_source: Some(crate::TriggerSource::LaboratoryResult),
        trigger_ref: Some(input.result_revision_id.to_string()),
        reason: input.reason,
    };
    request_follow_on_at_boundary(ctx, request, TriggerBoundary::VerifiedLaboratoryResult)
}

fn request_follow_on_at_boundary(
    ctx: &mut CommandContext<'_>,
    input: RequestFollowOnInput,
    boundary: TriggerBoundary,
) -> Result<crate::OrderDetail, OrderError> {
    require_at_boundary(ctx, boundary)?;
    reason("reason", &input.reason)?;
    optional("authorityReference", input.authority_reference.as_deref())?;
    if input.kind == ItemKind::Initial {
        return Err(invalid(
            "kind",
            "follow_on_required",
            "Choose add-on or reflex.",
        ));
    }
    let version = active_orderable(ctx.tx(), input.orderable_version_id)?;
    validate_follow_on_trigger(ctx, &input, boundary)?;
    if input.kind == ItemKind::Reflex
        && let Some(existing) = replayed_activation(ctx, &input, version.orderable_id, boundary)?
    {
        return get_order(ctx.tx(), existing);
    }
    let (status, revision) = load_order_state(ctx, input.order_id)?;
    if revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if status != OrderStatus::Accepted {
        return Err(OrderError::InvalidTransition);
    }
    if input.kind == ItemKind::Reflex {
        ensure_reflex_scope(
            ctx,
            input.order_id,
            input.parent_item_id,
            version.orderable_id,
        )?;
    } else if let Some(parent_id) = input.parent_item_id {
        ensure_parent_order(ctx, input.order_id, parent_id)?;
    }
    let item_id = OrderItemId::generate();
    ctx.tx().execute(
        "INSERT INTO ord_order_items (id, order_id, orderable_version_id, kind, sequence_role, parent_item_id, \
         reflex_authorization, authority_reference, state, requested_at, reason, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'not_stated', ?7, 'requested', ?8, ?9, 1)",
        params![
            item_id,
            input.order_id,
            input.orderable_version_id,
            input.kind,
            input.sequence_role,
            input.parent_item_id,
            input.authority_reference,
            ctx.now(),
            input.reason
        ],
    )?;
    if input.kind == ItemKind::Reflex {
        insert_activation(ctx, &input, version.orderable_id, item_id, boundary)?;
    }
    let next = revision + 1;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "order.request_follow_on",
            Outcome::Succeeded,
        )
        .record("order_item", item_id)
        .new_revision(1)
        .reason(input.reason)
        .detail("order_id", AuditDetail::id(input.order_id))
        .detail("kind", AuditDetail::code(input.kind.as_str())),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = ?2 WHERE id = ?1",
        params![input.order_id, next],
    )?;
    get_order(ctx.tx(), input.order_id)
}

/// Accepts or declines a pending follow-on; acceptance freezes its components.
pub fn decide_follow_on(
    ctx: &mut CommandContext<'_>,
    input: DecideFollowOnInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    reason("reason", &input.reason)?;
    let (order_id, version_id, kind, sequence_role, authority_reference, state, revision): (
        OrderId,
        lis_catalog::OrderableVersionId,
        ItemKind,
        crate::SequenceRole,
        Option<String>,
        ItemState,
        i64,
    ) = ctx
        .tx()
        .query_row(
            "SELECT order_id, orderable_version_id, kind, sequence_role, authority_reference, state, revision \
             FROM ord_order_items WHERE id = ?1",
            [input.item_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?
        .ok_or(OrderError::NotFound("order_item"))?;
    if revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if state != ItemState::Requested || kind == ItemKind::Initial {
        return Err(OrderError::InvalidTransition);
    }
    if input.decision == Decision::Accept
        && kind == ItemKind::AddOn
        && authority_reference.is_none_or(|reference| reference.trim().is_empty())
    {
        return Err(OrderError::GateBlocked);
    }
    if input.decision == Decision::Accept {
        freeze_components(ctx, input.item_id, version_id, sequence_role)?;
    }
    let next_state = match input.decision {
        Decision::Accept => "accepted",
        Decision::Decline => "declined",
    };
    let action = "order.decide_follow_on";
    ctx.audit(
        AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded)
            .record("order_item", input.item_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(input.reason.clone())
            .detail("decision", AuditDetail::code(input.decision.as_str())),
    )?;
    ctx.tx().execute(
        "UPDATE ord_order_items SET state = ?2, decided_at = ?3, decided_by = ?4, reason = ?5, \
         revision = revision + 1 WHERE id = ?1",
        params![
            input.item_id,
            next_state,
            ctx.now(),
            ctx.principal().actor().id_text(),
            input.reason
        ],
    )?;
    bump_order(ctx, order_id)?;
    get_order(ctx.tx(), order_id)
}
