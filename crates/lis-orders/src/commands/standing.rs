//! Patient-specific standing orders and their explicit scope.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::params;

use crate::error::{OrderError, invalid};
use crate::inputs::RegisterStandingOrderInput;
use crate::queries::list_standing_orders;
use crate::{StandingOrderId, StandingOrderView};

use super::validation::{iso_date, orderable_exists, required};

/// Registers a patient-specific standing order. It never creates patient
/// orders automatically (REQ-ORD-006; INV-13).
pub fn register_standing_order(
    ctx: &mut CommandContext<'_>,
    input: RegisterStandingOrderInput,
) -> Result<StandingOrderView, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    iso_date("validFrom", &input.valid_from)?;
    iso_date("validUntil", &input.valid_until)?;
    if input.valid_from > input.valid_until {
        return Err(invalid(
            "validUntil",
            "before_valid_from",
            "The end date must not be before the start date.",
        ));
    }
    if input.visit_limit == Some(0) {
        return Err(invalid(
            "visitLimit",
            "out_of_range",
            "Use a positive visit limit or leave it empty.",
        ));
    }
    required("documentRef", &input.document_ref)?;
    if input.scope_orderable_ids.is_empty() {
        return Err(invalid(
            "scopeOrderableIds",
            "required",
            "Add at least one orderable to the scope.",
        ));
    }
    let mut scope = BTreeSet::new();
    for (index, id) in input.scope_orderable_ids.iter().enumerate() {
        if !scope.insert(*id) {
            return Err(invalid(
                &format!("scopeOrderableIds[{index}]"),
                "duplicate",
                "This orderable is listed twice.",
            ));
        }
        orderable_exists(ctx.tx(), *id)?;
    }
    // No client applies to a standing-order record, so use public identity APIs directly.
    lis_identity::get_subject(ctx.tx(), input.subject_id)?;
    if !lis_identity::list_active_practitioners(ctx.tx())?
        .iter()
        .any(|practitioner| practitioner.id == input.practitioner_id)
    {
        return Err(OrderError::NotFound("practitioner"));
    }
    let id = StandingOrderId::generate();
    ctx.tx().execute(
        "INSERT INTO ord_standing_orders (id, subject_id, practitioner_id, valid_from, valid_until, \
         visit_limit, document_ref, recorded_by, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, input.subject_id, input.practitioner_id, input.valid_from, input.valid_until,
            input.visit_limit, input.document_ref, ctx.principal().actor().id_text(), ctx.now()],
    )?;
    for orderable_id in input.scope_orderable_ids {
        ctx.tx().execute(
            "INSERT INTO ord_standing_order_scope (standing_order_id, orderable_id) VALUES (?1, ?2)",
            params![id, orderable_id],
        )?;
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "order.register_standing_order",
            Outcome::Succeeded,
        )
        .record("standing_order", id)
        .new_revision(1)
        .detail("subject_id", AuditDetail::id(input.subject_id))
        .detail("scope_count", AuditDetail::count(scope.len()))
        .detail(
            "visit_limited",
            AuditDetail::boolean(input.visit_limit.is_some()),
        ),
    )?;
    list_standing_orders(ctx.tx(), input.subject_id)?
        .into_iter()
        .find(|standing| standing.standing_order_id == id)
        .ok_or(OrderError::NotFound("standing_order"))
}
