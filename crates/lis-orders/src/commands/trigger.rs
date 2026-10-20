//! Reflex trigger provenance, scope gates and idempotent assessments.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_catalog::OrderableId;
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};

use crate::error::{OrderError, invalid};
use crate::inputs::{
    LaboratoryResultTriggerNotMetInput, RecordTriggerNotMetInput, RequestFollowOnInput,
};
use crate::queries::get_order;
use crate::{
    ItemKind, ItemState, OrderId, OrderItemId, ReflexAuthorization, TriggerAssessmentId,
    TriggerSource,
};

use super::order_support::bump_order;
use super::validation::{reason, required};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TriggerBoundary {
    Public,
    VerifiedLaboratoryResult,
}

/// Records an idempotent condition-not-met trigger decision. It creates no
/// item, charge or negative laboratory result (REQ-ORD-003).
pub fn record_trigger_not_met(
    ctx: &mut CommandContext<'_>,
    input: RecordTriggerNotMetInput,
) -> Result<crate::OrderDetail, OrderError> {
    record_trigger_not_met_at_boundary(ctx, input, TriggerBoundary::Public)
}

/// Records a condition-not-met assessment for a result revision verified by its owner.
///
/// Before calling, the results owner must authorize the caller and verify the
/// current, reviewed result revision and its order in this same transaction.
/// This entry point refuses a parent item outside that order; scope and
/// replay checks are the public command's.
pub fn record_trigger_not_met_from_result(
    ctx: &mut CommandContext<'_>,
    input: LaboratoryResultTriggerNotMetInput,
) -> Result<crate::OrderDetail, OrderError> {
    if parent_order(ctx, input.parent_item_id)? != input.order_id {
        return Err(OrderError::GateBlocked);
    }
    let assessment = RecordTriggerNotMetInput {
        parent_item_id: input.parent_item_id,
        orderable_id: input.orderable_id,
        trigger_source: TriggerSource::LaboratoryResult,
        trigger_ref: input.result_revision_id.to_string(),
        reason: input.reason,
    };
    record_trigger_not_met_at_boundary(ctx, assessment, TriggerBoundary::VerifiedLaboratoryResult)
}

fn record_trigger_not_met_at_boundary(
    ctx: &mut CommandContext<'_>,
    input: RecordTriggerNotMetInput,
    boundary: TriggerBoundary,
) -> Result<crate::OrderDetail, OrderError> {
    require_at_boundary(ctx, boundary)?;
    if input.trigger_source == TriggerSource::LaboratoryResult
        && boundary == TriggerBoundary::Public
    {
        return Err(invalid(
            "triggerSource",
            "not_allowed",
            "Laboratory-result triggers must come from the results workflow.",
        ));
    }
    required("triggerRef", &input.trigger_ref)?;
    reason("reason", &input.reason)?;
    let order_id = parent_order(ctx, input.parent_item_id)?;
    if input.trigger_source == TriggerSource::ClientPointOfCare {
        ensure_poc_reference(ctx, order_id, &input.trigger_ref)?;
    }
    ensure_reflex_scope(
        ctx,
        order_id,
        Some(input.parent_item_id),
        input.orderable_id,
    )?;
    if let Some((outcome, source, trigger_ref)) =
        existing_trigger(ctx, input.parent_item_id, input.orderable_id)?
    {
        if outcome == "condition_not_met"
            && source == input.trigger_source
            && trigger_ref == input.trigger_ref
        {
            return get_order(ctx.tx(), order_id);
        }
        return Err(OrderError::ReplayConflict);
    }
    let id = TriggerAssessmentId::generate();
    ctx.tx().execute(
        "INSERT INTO ord_trigger_assessments (id, parent_item_id, orderable_id, outcome, trigger_source, \
         trigger_ref, reason, decided_by, decided_at) VALUES (?1, ?2, ?3, 'condition_not_met', ?4, ?5, ?6, ?7, ?8)",
        params![id, input.parent_item_id, input.orderable_id, input.trigger_source, input.trigger_ref,
            input.reason, ctx.principal().actor().id_text(), ctx.now()],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "order.trigger_not_met",
            Outcome::Succeeded,
        )
        .record("trigger_assessment", id)
        .new_revision(1)
        .reason(input.reason)
        .detail("order_id", AuditDetail::id(order_id))
        .detail(
            "trigger_source",
            AuditDetail::code(input.trigger_source.as_str()),
        ),
    )?;
    bump_order(ctx, order_id)?;
    get_order(ctx.tx(), order_id)
}

/// Public commands need `enter_orders`. At the verified laboratory-result
/// boundary the results caller has already checked its own capability
/// (`review_results` or `enter_orders`) in this transaction.
pub(super) fn require_at_boundary(
    ctx: &mut CommandContext<'_>,
    boundary: TriggerBoundary,
) -> Result<(), OrderError> {
    match boundary {
        TriggerBoundary::Public => Ok(ctx.require(Capability::EnterOrders)?),
        TriggerBoundary::VerifiedLaboratoryResult => Ok(()),
    }
}

pub(super) fn ensure_reflex_scope(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    parent_id: Option<OrderItemId>,
    orderable_id: OrderableId,
) -> Result<(), OrderError> {
    let parent_id = parent_id.ok_or_else(|| {
        invalid(
            "parentItemId",
            "required",
            "A reflex needs its parent item.",
        )
    })?;
    let (parent_order, state, authority): (OrderId, ItemState, ReflexAuthorization) = ctx
        .tx()
        .query_row(
            "SELECT order_id, state, reflex_authorization FROM ord_order_items WHERE id = ?1",
            [parent_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("parent_order_item"))?;
    let in_scope = ctx
        .tx()
        .query_row(
            "SELECT 1 FROM ord_reflex_scope WHERE item_id = ?1 AND orderable_id = ?2",
            params![parent_id, orderable_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if parent_order != order_id
        || state != ItemState::Accepted
        || authority != ReflexAuthorization::Authorized
        || !in_scope
    {
        return Err(OrderError::GateBlocked);
    }
    Ok(())
}

pub(super) fn ensure_parent_order(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    parent_id: OrderItemId,
) -> Result<(), OrderError> {
    if parent_order(ctx, parent_id)? == order_id {
        Ok(())
    } else {
        Err(OrderError::NotFound("parent_order_item"))
    }
}

fn parent_order(ctx: &CommandContext<'_>, parent_id: OrderItemId) -> Result<OrderId, OrderError> {
    ctx.tx()
        .query_row(
            "SELECT order_id FROM ord_order_items WHERE id = ?1",
            [parent_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(OrderError::NotFound("parent_order_item"))
}

fn trigger_identity(
    input: &RequestFollowOnInput,
    boundary: TriggerBoundary,
) -> Result<(TriggerSource, String), OrderError> {
    match (
        input.trigger_source,
        input.trigger_ref.as_deref(),
        input.poc_result_id,
    ) {
        (Some(TriggerSource::LaboratoryResult), Some(reference), None)
            if boundary == TriggerBoundary::VerifiedLaboratoryResult =>
        {
            required("triggerRef", reference)?;
            Ok((TriggerSource::LaboratoryResult, reference.to_owned()))
        }
        (Some(TriggerSource::ClientPointOfCare) | None, reference, Some(id))
            if reference.is_none_or(|value| value == id.to_string()) =>
        {
            Ok((TriggerSource::ClientPointOfCare, id.to_string()))
        }
        _ => Err(invalid(
            "triggerRef",
            "required",
            "A reflex needs its trigger source and reference.",
        )),
    }
}

pub(super) fn validate_follow_on_trigger(
    ctx: &CommandContext<'_>,
    input: &RequestFollowOnInput,
    boundary: TriggerBoundary,
) -> Result<(), OrderError> {
    if input.kind != ItemKind::Reflex {
        if input.trigger_source.is_some()
            || input.trigger_ref.is_some()
            || input.poc_result_id.is_some()
        {
            return Err(invalid(
                "triggerRef",
                "not_allowed",
                "Trigger evidence applies only to a reflex.",
            ));
        }
        return Ok(());
    }
    if input.trigger_source == Some(TriggerSource::LaboratoryResult)
        && boundary == TriggerBoundary::Public
    {
        return Err(invalid(
            "triggerSource",
            "not_allowed",
            "Laboratory-result triggers must come from the results workflow.",
        ));
    }
    let (source, reference) = trigger_identity(input, boundary)?;
    if source == TriggerSource::ClientPointOfCare {
        ensure_poc_reference(ctx, input.order_id, &reference)?;
    }
    Ok(())
}

fn ensure_poc_reference(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    reference: &str,
) -> Result<(), OrderError> {
    let exists = ctx
        .tx()
        .query_row(
            "SELECT 1 FROM ord_poc_results WHERE id = ?1 AND order_id = ?2 \
             AND removed_in_version IS NULL",
            params![reference, order_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(OrderError::GateBlocked)
    }
}

pub(super) fn replayed_activation(
    ctx: &CommandContext<'_>,
    input: &RequestFollowOnInput,
    orderable_id: OrderableId,
    boundary: TriggerBoundary,
) -> Result<Option<OrderId>, OrderError> {
    let Some(parent_id) = input.parent_item_id else {
        return Ok(None);
    };
    let Some((outcome, source, reference)) = existing_trigger(ctx, parent_id, orderable_id)? else {
        return Ok(None);
    };
    let expected = trigger_identity(input, boundary)?;
    if outcome == "activated" && source == expected.0 && reference == expected.1 {
        Ok(Some(parent_order(ctx, parent_id)?))
    } else {
        Err(OrderError::ReplayConflict)
    }
}

pub(super) fn insert_activation(
    ctx: &CommandContext<'_>,
    input: &RequestFollowOnInput,
    orderable_id: OrderableId,
    created_item_id: OrderItemId,
    boundary: TriggerBoundary,
) -> Result<(), OrderError> {
    let parent_id = input.parent_item_id.ok_or(OrderError::GateBlocked)?;
    let (source, reference) = trigger_identity(input, boundary)?;
    ctx.tx().execute(
        "INSERT INTO ord_trigger_assessments (id, parent_item_id, orderable_id, outcome, \
         trigger_source, trigger_ref, created_item_id, reason, decided_by, decided_at) \
         VALUES (?1, ?2, ?3, 'activated', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            TriggerAssessmentId::generate(),
            parent_id,
            orderable_id,
            source,
            reference,
            created_item_id,
            input.reason,
            ctx.principal().actor().id_text(),
            ctx.now()
        ],
    )?;
    Ok(())
}

fn existing_trigger(
    ctx: &CommandContext<'_>,
    parent_id: OrderItemId,
    orderable_id: OrderableId,
) -> Result<Option<(String, TriggerSource, String)>, OrderError> {
    Ok(ctx
        .tx()
        .query_row(
            "SELECT outcome, trigger_source, trigger_ref FROM ord_trigger_assessments \
             WHERE parent_item_id = ?1 AND orderable_id = ?2",
            params![parent_id, orderable_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?)
}
