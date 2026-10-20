//! Persistence and gate helpers shared by order registration and decisions.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_catalog::{OrderableId, OrderableVersionId};
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};

use crate::error::OrderError;
use crate::inputs::{AcceptOrderInput, RegisterOrderInput, TemplateUseInput};
use crate::{
    ClarificationDeficiency, ClarificationHoldId, OrderComponentId, OrderId, OrderItemId,
    OrderStatus, SequenceRole,
};

use super::validation::active_orderable;

pub(super) fn insert_registered_order(
    ctx: &CommandContext<'_>,
    input: &RegisterOrderInput,
    versions: &[lis_catalog::OrderableVersionView],
) -> Result<OrderId, OrderError> {
    let order_id = OrderId::generate();
    let actor = ctx.principal().actor();
    let (template_use, template_name, template_confirmed) = match &input.template_use {
        TemplateUseInput::None => ("none", None, None),
        TemplateUseInput::ClinicTemplate {
            name,
            confirmed_per_patient,
        } => (
            "clinic_template",
            Some(name.as_str()),
            Some(*confirmed_per_patient),
        ),
    };
    ctx.tx().execute(
        "INSERT INTO ord_orders (id, subject_id, client_id, client_location_id, practitioner_id, \
         placer_order_id, placer_authority, source_kind, source_document_ref, billing_responsibility, \
         facility_setting, standing_order_id, template_use, template_name, template_confirmed, status, \
         version, received_at, received_by_kind, received_by_id, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, \
         'received', 1, ?16, ?17, ?18, 1)",
        params![order_id, input.subject_id, input.client_id, input.client_location_id,
            input.practitioner_id, input.placer_order_id, input.placer_authority, input.source_kind,
            input.source_document_ref, input.billing_responsibility, input.facility_setting,
            input.standing_order_id, template_use, template_name, template_confirmed, ctx.now(),
            actor.kind(), actor.id_text()],
    )?;
    for (item, version) in input.items.iter().zip(versions) {
        let item_id = OrderItemId::generate();
        ctx.tx().execute(
            "INSERT INTO ord_order_items (id, order_id, orderable_version_id, kind, sequence_role, \
             reflex_authorization, state, requested_at, revision) \
             VALUES (?1, ?2, ?3, 'initial', ?4, ?5, 'requested', ?6, 1)",
            params![
                item_id,
                order_id,
                version.id,
                item.sequence_role,
                item.reflex_authorization,
                ctx.now()
            ],
        )?;
        for orderable_id in &item.reflex_scope_orderable_ids {
            ctx.tx().execute(
                "INSERT INTO ord_reflex_scope (item_id, orderable_id) VALUES (?1, ?2)",
                params![item_id, orderable_id],
            )?;
        }
    }
    Ok(order_id)
}

pub(super) fn load_acceptance_state(
    ctx: &CommandContext<'_>,
    input: &AcceptOrderInput,
) -> Result<(i64, Option<crate::StandingOrderId>), OrderError> {
    let (status, revision, standing_order_id) = ctx
        .tx()
        .query_row(
            "SELECT status, revision, standing_order_id FROM ord_orders WHERE id = ?1",
            [input.order_id],
            |row| Ok((row.get::<_, OrderStatus>(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))?;
    if revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if status != OrderStatus::Received {
        return Err(OrderError::InvalidTransition);
    }
    Ok((revision, standing_order_id))
}

pub(super) fn ensure_no_open_holds(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
) -> Result<(), OrderError> {
    let open_holds: i64 = ctx.tx().query_row(
        "SELECT count(*) FROM ord_clarification_holds WHERE order_id = ?1 AND state = 'open'",
        [order_id],
        |row| row.get(0),
    )?;
    if open_holds == 0 {
        Ok(())
    } else {
        Err(OrderError::GateBlocked)
    }
}

pub(super) fn has_active_authority(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
) -> Result<bool, OrderError> {
    let supplied: i64 = ctx.tx().query_row(
        "SELECT count(*) FROM ord_authority_evidence \
         WHERE order_id = ?1 AND removed_in_version IS NULL AND kind <> 'not_supplied'",
        [order_id],
        |row| row.get(0),
    )?;
    Ok(supplied != 0)
}

/// Adds the missing-authority hold once and commits it as the acceptance outcome.
pub(super) fn apply_authority_hold(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    revision: i64,
) -> Result<(), OrderError> {
    let already_open = ctx
        .tx()
        .query_row(
            "SELECT 1 FROM ord_clarification_holds \
             WHERE order_id = ?1 AND deficiency = 'signature_or_intent_required' \
             AND state = 'open'",
            [order_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if already_open {
        return Ok(());
    }
    let actor = ctx.principal().actor().id_text();
    insert_hold(
        ctx,
        order_id,
        ClarificationDeficiency::SignatureOrIntentRequired,
        "user",
        None,
        &actor,
    )?;
    let next_revision = revision + 1;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.hold", Outcome::Succeeded)
            .record("order", order_id)
            .old_revision(revision)
            .new_revision(next_revision)
            .detail(
                "deficiency",
                AuditDetail::code(ClarificationDeficiency::SignatureOrIntentRequired.as_str()),
            ),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = ?2 WHERE id = ?1",
        params![order_id, next_revision],
    )?;
    Ok(())
}

pub(super) fn load_order_state(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
) -> Result<(OrderStatus, i64), OrderError> {
    ctx.tx()
        .query_row(
            "SELECT status, revision FROM ord_orders WHERE id = ?1",
            [order_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))
}

pub(super) fn apply_standing_hold(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    revision: i64,
    deficiency: ClarificationDeficiency,
) -> Result<(), OrderError> {
    let next_revision = revision + 1;
    insert_hold(
        ctx,
        order_id,
        deficiency,
        "standing_order_check",
        None,
        "standing-order-check",
    )?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.hold", Outcome::Succeeded)
            .record("order", order_id)
            .old_revision(revision)
            .new_revision(next_revision)
            .detail("deficiency", AuditDetail::code(deficiency.as_str())),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = ?2 WHERE id = ?1",
        params![order_id, next_revision],
    )?;
    Ok(())
}

pub(super) fn freeze_components(
    ctx: &CommandContext<'_>,
    item_id: OrderItemId,
    version_id: lis_catalog::OrderableVersionId,
    sequence_role: SequenceRole,
) -> Result<(), OrderError> {
    let version = active_orderable(ctx.tx(), version_id)?;
    for component in version.components {
        ctx.tx().execute(
            "INSERT INTO ord_order_components (id, item_id, catalog_orderable_version_id, \
             catalog_component_position, specimen_type_code, purpose, sequence_role, state, revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', 1)",
            params![OrderComponentId::generate(), item_id, version_id, component.position,
                component.specimen_type_code, component.purpose, sequence_role],
        )?;
    }
    Ok(())
}

pub(super) fn ensure_order_revision(
    status: OrderStatus,
    current: i64,
    expected: i64,
) -> Result<(), OrderError> {
    if current != expected {
        return Err(OrderError::RevisionConflict);
    }
    if status != OrderStatus::Received {
        return Err(OrderError::InvalidTransition);
    }
    Ok(())
}

pub(super) fn bump_order(ctx: &CommandContext<'_>, order_id: OrderId) -> Result<(), OrderError> {
    ctx.tx().execute(
        "UPDATE ord_orders SET revision = revision + 1 WHERE id = ?1",
        [order_id],
    )?;
    Ok(())
}

pub(super) fn insert_hold(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    deficiency: ClarificationDeficiency,
    origin: &str,
    note: Option<String>,
    opened_by: &str,
) -> Result<(), OrderError> {
    ctx.tx().execute(
        "INSERT INTO ord_clarification_holds (id, order_id, deficiency, origin, note, state, \
         opened_by, opened_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, 1)",
        params![
            ClarificationHoldId::generate(),
            order_id,
            deficiency,
            origin,
            note,
            opened_by,
            ctx.now()
        ],
    )?;
    Ok(())
}

pub(super) fn standing_order_deficiency(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    standing_id: crate::StandingOrderId,
) -> Result<Option<ClarificationDeficiency>, OrderError> {
    let (subject_id, practitioner_id, visit_limit): (
        lis_identity::SubjectId,
        lis_identity::PractitionerId,
        Option<i64>,
    ) = ctx
        .tx()
        .query_row(
            "SELECT subject_id, practitioner_id, visit_limit \
         FROM ord_standing_orders WHERE id = ?1",
            [standing_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("standing_order"))?;
    let (order_subject, order_practitioner): (
        lis_identity::SubjectId,
        lis_identity::PractitionerId,
    ) = ctx.tx().query_row(
        "SELECT subject_id, practitioner_id FROM ord_orders WHERE id = ?1",
        [order_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if subject_id != order_subject || practitioner_id != order_practitioner {
        return Ok(Some(ClarificationDeficiency::StandingOrderExpired));
    }
    if let Some(limit) = visit_limit {
        let accepted: i64 = ctx.tx().query_row(
            "SELECT count(*) FROM ord_orders WHERE standing_order_id = ?1 AND status = 'accepted'",
            [standing_id],
            |r| r.get(0),
        )?;
        if accepted >= limit {
            return Ok(Some(ClarificationDeficiency::StandingOrderExhausted));
        }
    }
    let scope: BTreeSet<OrderableId> = ctx
        .tx()
        .prepare_cached(
            "SELECT orderable_id FROM ord_standing_order_scope WHERE standing_order_id = ?1",
        )?
        .query_map([standing_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let versions: Vec<OrderableVersionId> = ctx
        .tx()
        .prepare_cached("SELECT orderable_version_id FROM ord_order_items WHERE order_id = ?1")?
        .query_map([order_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let mut outside_scope = false;
    for version_id in versions {
        let version = lis_catalog::get_orderable_version(ctx.tx(), version_id)?;
        outside_scope |= !scope.contains(&version.orderable_id);
    }
    if outside_scope {
        return Ok(Some(ClarificationDeficiency::StandingOrderOutOfScope));
    }
    Ok(None)
}
