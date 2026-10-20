//! Versioned order evidence, retained exactly as supplied (INV-13).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::params;

use crate::error::OrderError;
use crate::inputs::{AmendOrderEvidenceInput, EvidenceInput};
use crate::queries::get_order;
use crate::{OrderEvidenceId, OrderId, OrderStatus};

use super::order_support::load_order_state;
use super::validation::{reason, validate_evidence};

/// Adds/retire evidence as a new immutable order evidence version (REQ-ORD-005/007).
pub fn amend_order_evidence(
    ctx: &mut CommandContext<'_>,
    input: AmendOrderEvidenceInput,
) -> Result<crate::OrderDetail, OrderError> {
    ctx.require(Capability::EnterOrders)?;
    reason("reason", &input.reason)?;
    validate_evidence(&input.add)?;
    let (status, revision) = load_order_state(ctx, input.order_id)?;
    let version: i64 = ctx.tx().query_row(
        "SELECT version FROM ord_orders WHERE id = ?1",
        [input.order_id],
        |row| row.get(0),
    )?;
    if revision != input.expected_revision {
        return Err(OrderError::RevisionConflict);
    }
    if status == OrderStatus::Rejected {
        return Err(OrderError::InvalidTransition);
    }
    let next_version = version + 1;
    retire_evidence(
        ctx,
        input.order_id,
        &input.remove_evidence_ids,
        next_version,
    )?;
    insert_evidence(ctx, input.order_id, next_version, &input.add)?;
    let next_revision = revision + 1;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "order.amend_evidence", Outcome::Succeeded)
            .record("order", input.order_id)
            .old_revision(revision)
            .new_revision(next_revision)
            .reason(input.reason)
            .detail("evidence_version", AuditDetail::integer(next_version)),
    )?;
    ctx.tx().execute(
        "UPDATE ord_orders SET version = ?2, revision = ?3 WHERE id = ?1",
        params![input.order_id, next_version, next_revision],
    )?;
    get_order(ctx.tx(), input.order_id)
}

pub(super) fn insert_evidence(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    version: i64,
    input: &EvidenceInput,
) -> Result<(), OrderError> {
    ctx.tx().execute(
        "INSERT INTO ord_authority_evidence (id, order_id, kind, document_ref, added_in_version) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            OrderEvidenceId::generate(),
            order_id,
            input.authority.kind,
            input.authority.document_ref,
            version
        ],
    )?;
    for diagnosis in &input.diagnoses {
        ctx.tx().execute(
            "INSERT INTO ord_diagnoses (id, order_id, code, code_system, code_system_version, \
             document_ref, preprinted_on_lab_form, added_in_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![OrderEvidenceId::generate(), order_id, diagnosis.code, diagnosis.code_system,
                diagnosis.code_system_version, diagnosis.document_ref, diagnosis.preprinted_on_lab_form, version],
        )?;
    }
    for medication in &input.medications {
        ctx.tx().execute(
            "INSERT INTO ord_medication_statements (id, order_id, received_text, code, code_system, \
             source, reporter, provided_on, as_of_date, added_in_version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![OrderEvidenceId::generate(), order_id, medication.received_text, medication.code,
                medication.code_system, medication.source, medication.reporter, medication.provided_on,
                medication.as_of_date, version],
        )?;
    }
    for result in &input.point_of_care_results {
        ctx.tx().execute(
            "INSERT INTO ord_poc_results (id, order_id, device_or_test, analyte_text, result_text, \
             performed_at_text, source, added_in_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                OrderEvidenceId::generate(),
                order_id,
                result.device_or_test,
                result.analyte_text,
                result.result_text,
                result.performed_at_text,
                result.source,
                version
            ],
        )?;
    }
    Ok(())
}

fn retire_evidence(
    ctx: &CommandContext<'_>,
    order_id: OrderId,
    ids: &[OrderEvidenceId],
    version: i64,
) -> Result<(), OrderError> {
    for id in ids {
        let changed = ctx.tx().execute(
            "UPDATE ord_authority_evidence SET removed_in_version = ?3 \
             WHERE id = ?1 AND order_id = ?2 AND removed_in_version IS NULL",
            params![id, order_id, version],
        )? + ctx.tx().execute(
            "UPDATE ord_diagnoses SET removed_in_version = ?3 \
             WHERE id = ?1 AND order_id = ?2 AND removed_in_version IS NULL",
            params![id, order_id, version],
        )? + ctx.tx().execute(
            "UPDATE ord_medication_statements SET removed_in_version = ?3 \
             WHERE id = ?1 AND order_id = ?2 AND removed_in_version IS NULL",
            params![id, order_id, version],
        )? + ctx.tx().execute(
            "UPDATE ord_poc_results SET removed_in_version = ?3 \
             WHERE id = ?1 AND order_id = ?2 AND removed_in_version IS NULL",
            params![id, order_id, version],
        )?;
        if changed != 1 {
            return Err(OrderError::NotFound("active_order_evidence"));
        }
    }
    Ok(())
}
