//! Registration, order decisions and clarification/billing tasks.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};

use crate::error::OrderError;
use crate::inputs::{
    AcceptOrderInput, CreateBillingReviewItemInput, PlaceClarificationHoldInput,
    RegisterOrderInput, RejectOrderInput, ResolveClarificationHoldInput, TemplateUseInput,
};
use crate::queries::get_order;
use crate::{BillingReviewItemId, OrderId, OrderItemId, OrderStatus};

use super::evidence::insert_evidence;
use super::order_support::{
    apply_authority_hold, apply_standing_hold, ensure_no_open_holds, ensure_order_revision,
    freeze_components, has_active_authority, insert_hold, insert_registered_order,
    load_acceptance_state, load_order_state, standing_order_deficiency,
};
use super::validation::{
    active_orderable, optional, reason, validate_references, validate_registration,
};

/// Records an order and all supplied evidence in `received` state (REQ-ORD-001/005–007).
pub fn register_order(
    ctx: &mut CommandContext<'_>,
    input: RegisterOrderInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    validate_registration(&input)?;
    validate_references(
        ctx.tx(),
        input.subject_id,
        input.client_id,
        input.client_location_id,
        input.practitioner_id,
    )?;
    lis_identity::assert_active_ordering_source(
        ctx.tx(),
        input.client_id,
        input.client_location_id,
        input.practitioner_id,
    )?;
    let mut versions = Vec::with_capacity(input.items.len());
    for item in &input.items {
        versions.push(active_orderable(ctx.tx(), item.orderable_version_id)?);
    }
    if let Some(standing_id) = input.standing_order_id {
        let exists = ctx
            .tx()
            .query_row(
                "SELECT 1 FROM ord_standing_orders WHERE id = ?1 AND subject_id = ?2",
                params![standing_id, input.subject_id],
                |_| Ok(()),
            )
            .optional()?;
        if exists.is_none() {
            return Err(OrderError::NotFound("standing_order"));
        }
    }
    let order_id = insert_registered_order(ctx, &input, &versions)?;
    insert_evidence(ctx, order_id, 1, &input.evidence)?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.register", Outcome::Succeeded)
            .record("order", order_id)
            .new_revision(1)
            .detail("item_count", AuditDetail::count(input.items.len()))
            .detail(
                "standing_order",
                AuditDetail::boolean(input.standing_order_id.is_some()),
            )
            .detail(
                "template_used",
                AuditDetail::boolean(matches!(
                    input.template_use,
                    TemplateUseInput::ClinicTemplate { .. }
                )),
            ),
    )?;
    get_order(ctx.tx(), order_id)
}

/// Accepts a received order and freezes its catalog components (REQ-ORD-001/002/006).
/// A failed standing-order recheck commits a system-originated hold and returns normally.
pub fn accept_order(
    ctx: &mut CommandContext<'_>,
    input: AcceptOrderInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    let (revision, standing_order_id) = load_acceptance_state(ctx, &input)?;
    let legacy_item: bool = ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM ord_order_items \
         WHERE order_id = ?1 AND state = 'requested' AND sequence_role IS NULL)",
        [input.order_id],
        |row| row.get(0),
    )?;
    if legacy_item {
        return Err(OrderError::LegacySequenceRoleMissing);
    }
    if !has_active_authority(ctx, input.order_id)? {
        apply_authority_hold(ctx, input.order_id, revision)?;
        return get_order(ctx.tx(), input.order_id);
    }
    ensure_no_open_holds(ctx, input.order_id)?;
    if let Some(standing_order_id) = standing_order_id
        && let Some(deficiency) = standing_order_deficiency(ctx, input.order_id, standing_order_id)?
    {
        apply_standing_hold(ctx, input.order_id, revision, deficiency)?;
        return get_order(ctx.tx(), input.order_id);
    }
    let items: Vec<(
        OrderItemId,
        lis_catalog::OrderableVersionId,
        crate::SequenceRole,
    )> = ctx
        .tx()
        .prepare_cached(
            "SELECT id, orderable_version_id, sequence_role FROM ord_order_items \
             WHERE order_id = ?1 AND state = 'requested' ORDER BY id",
        )?
        .query_map([input.order_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    for (item_id, version_id, sequence_role) in &items {
        freeze_components(ctx, *item_id, *version_id, *sequence_role)?;
    }
    let next_revision = revision + 1;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.accept", Outcome::Succeeded)
            .record("order", input.order_id)
            .old_revision(revision)
            .new_revision(next_revision)
            .detail("item_count", AuditDetail::count(items.len())),
    )?;
    let actor_id = ctx.principal().actor().id_text();
    ctx.tx().execute(
        "UPDATE ord_order_items SET state = 'accepted', decided_at = ?2, decided_by = ?3, \
         revision = revision + 1 WHERE order_id = ?1 AND state = 'requested'",
        params![input.order_id, ctx.now(), actor_id],
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET status = 'accepted', decided_at = ?2, decided_by = ?3, \
         revision = ?4 WHERE id = ?1",
        params![input.order_id, ctx.now(), actor_id, next_revision],
    )?;
    get_order(ctx.tx(), input.order_id)
}

/// Rejects a received order with attributable reason.
pub fn reject_order(
    ctx: &mut CommandContext<'_>,
    input: RejectOrderInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    reason("reason", &input.reason)?;
    let (status, revision) = load_order_state(ctx, input.order_id)?;
    ensure_order_revision(status, revision, input.expected_revision)?;
    let next = revision + 1;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.reject", Outcome::Succeeded)
            .record("order", input.order_id)
            .old_revision(revision)
            .new_revision(next)
            .reason(input.reason.clone()),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET status = 'rejected', decided_at = ?2, decided_by = ?3, \
         reject_reason = ?4, revision = ?5 WHERE id = ?1",
        params![
            input.order_id,
            ctx.now(),
            ctx.principal().actor().id_text(),
            input.reason,
            next
        ],
    )?;
    get_order(ctx.tx(), input.order_id)
}

/// Places a clarification hold. Billing-only gaps use a billing review item instead.
pub fn place_clarification_hold(
    ctx: &mut CommandContext<'_>,
    input: PlaceClarificationHoldInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    optional("note", input.note.as_deref())?;
    let (status, revision) = load_order_state(ctx, input.order_id)?;
    if revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if status == OrderStatus::Rejected {
        return Err(OrderError::InvalidTransition);
    }
    let next = revision + 1;
    let actor = ctx.principal().actor().id_text();
    insert_hold(
        ctx,
        input.order_id,
        input.deficiency,
        "user",
        input.note,
        &actor,
    )?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.hold", Outcome::Succeeded)
            .record("order", input.order_id)
            .old_revision(revision)
            .new_revision(next)
            .detail("deficiency", AuditDetail::code(input.deficiency.as_str())),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = ?2 WHERE id = ?1",
        params![input.order_id, next],
    )?;
    get_order(ctx.tx(), input.order_id)
}

/// Resolves one open hold with a reason.
pub fn resolve_clarification_hold(
    ctx: &mut CommandContext<'_>,
    input: ResolveClarificationHoldInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    reason("reason", &input.reason)?;
    let (order_id, state, hold_revision): (OrderId, String, i64) = ctx
        .tx()
        .query_row(
            "SELECT order_id, state, revision FROM ord_clarification_holds WHERE id = ?1",
            [input.hold_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("clarification_hold"))?;
    if hold_revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if state != "open" {
        return Err(OrderError::InvalidTransition);
    }
    let order_revision: i64 = ctx.tx().query_row(
        "SELECT revision FROM ord_orders WHERE id = ?1",
        [order_id],
        |r| r.get(0),
    )?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.resolve_hold", Outcome::Succeeded)
            .record("clarification_hold", input.hold_id)
            .old_revision(hold_revision)
            .new_revision(hold_revision + 1)
            .reason(input.reason.clone())
            .detail("order_id", AuditDetail::id(order_id)),
    )?;
    ctx.tx().execute(
        "UPDATE ord_clarification_holds SET state = 'resolved', resolved_by = ?2, resolved_at = ?3, \
         resolution_reason = ?4, revision = revision + 1 WHERE id = ?1",
        params![input.hold_id, ctx.principal().actor().id_text(), ctx.now(), input.reason],
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = ?2 WHERE id = ?1",
        params![order_id, order_revision + 1],
    )?;
    get_order(ctx.tx(), order_id)
}

/// Opens non-blocking payer-documentation review (REQ-ORD-005).
pub fn create_billing_review_item(
    ctx: &mut CommandContext<'_>,
    input: CreateBillingReviewItemInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(lis_identity::require_financial_access(ctx.principal())?)?;
    optional("note", input.note.as_deref())?;
    let revision: i64 = ctx
        .tx()
        .query_row(
            "SELECT revision FROM ord_orders WHERE id = ?1",
            [input.order_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))?;
    let item_id = BillingReviewItemId::generate();
    ctx.tx().execute(
        "INSERT INTO ord_billing_review_items (id, order_id, element, note, opened_by, opened_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            item_id,
            input.order_id,
            input.element,
            input.note,
            ctx.principal().actor().id_text(),
            ctx.now()
        ],
    )?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.billing_review", Outcome::Succeeded)
            .record("billing_review_item", item_id)
            .new_revision(1)
            .detail("order_id", AuditDetail::id(input.order_id))
            .detail("element", AuditDetail::code(input.element.as_str())),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = ?2 WHERE id = ?1",
        params![input.order_id, revision + 1],
    )?;
    get_order(ctx.tx(), input.order_id)
}
