//! Hydrates the child collections of an order detail view.

use rusqlite::Connection;

use crate::OrderId;
use crate::error::OrderError;
use crate::views::{
    AuthorityEvidenceView, BillingReviewItemView, CancellationView, ClarificationHoldView,
    DiagnosisView, MedicationView, OrderComponentView, OrderDetail, OrderEvidenceView,
    OrderItemView, PointOfCareResultView, TriggerAssessmentView,
};

pub(super) fn order_items(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<OrderItemView>, OrderError> {
    let mut items: Vec<OrderItemView> = conn
        .prepare_cached(
            "SELECT id, kind, state, orderable_version_id, sequence_role, parent_item_id, reflex_authorization, \
             authority_reference, reason, revision FROM ord_order_items \
             WHERE order_id = ?1 ORDER BY requested_at, id",
        )?
        .query_map([order_id], |row| {
            Ok(OrderItemView {
                item_id: row.get(0)?,
                kind: row.get(1)?,
                state: row.get(2)?,
                orderable_version_id: row.get(3)?,
                orderable_code: String::new(),
                orderable_name: String::new(),
                sequence_role: row.get(4)?,
                parent_item_id: row.get(5)?,
                reflex_authorization: row.get(6)?,
                reflex_scope_orderable_ids: Vec::new(),
                authority_reference: row.get(7)?,
                reason: row.get(8)?,
                revision: row.get(9)?,
                components: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    for item in &mut items {
        let ordered = lis_catalog::get_orderable_version(conn, item.orderable_version_id)?;
        item.orderable_code = ordered.code;
        item.orderable_name = ordered.name;
        item.reflex_scope_orderable_ids = conn
            .prepare_cached(
                "SELECT orderable_id FROM ord_reflex_scope WHERE item_id = ?1 ORDER BY orderable_id",
            )?
            .query_map([item.item_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        item.components = conn
            .prepare_cached(
                "SELECT id, catalog_component_position, specimen_type_code, purpose, sequence_role, \
                 state, revision FROM ord_order_components WHERE item_id = ?1 \
                 ORDER BY catalog_component_position",
            )?
            .query_map([item.item_id], |row| {
                Ok(OrderComponentView {
                    component_id: row.get(0)?,
                    catalog_component_position: row.get(1)?,
                    specimen_type_code: row.get(2)?,
                    purpose: row.get(3)?,
                    sequence_role: row.get(4)?,
                    state: row.get(5)?,
                    revision: row.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
    }
    Ok(items)
}

pub(super) fn evidence(
    conn: &Connection,
    order_id: OrderId,
) -> Result<OrderEvidenceView, OrderError> {
    let authority = conn.prepare_cached("SELECT id, kind, document_ref, added_in_version, removed_in_version FROM ord_authority_evidence WHERE order_id = ?1 ORDER BY added_in_version, id")?
        .query_map([order_id], |r| Ok(AuthorityEvidenceView { evidence_id: r.get(0)?, kind: r.get(1)?, document_ref: r.get(2)?, added_in_version: r.get(3)?, removed_in_version: r.get(4)? }))?.collect::<Result<_, _>>()?;
    let diagnoses = conn.prepare_cached("SELECT id, code, code_system, code_system_version, document_ref, preprinted_on_lab_form, added_in_version, removed_in_version FROM ord_diagnoses WHERE order_id = ?1 ORDER BY added_in_version, id")?
        .query_map([order_id], |r| Ok(DiagnosisView { evidence_id: r.get(0)?, code: r.get(1)?, code_system: r.get(2)?, code_system_version: r.get(3)?, document_ref: r.get(4)?, preprinted_on_lab_form: r.get(5)?, added_in_version: r.get(6)?, removed_in_version: r.get(7)? }))?.collect::<Result<_, _>>()?;
    let medications = conn.prepare_cached("SELECT id, received_text, code, code_system, source, reporter, provided_on, as_of_date, added_in_version, removed_in_version FROM ord_medication_statements WHERE order_id = ?1 ORDER BY added_in_version, id")?
        .query_map([order_id], |r| Ok(MedicationView { evidence_id: r.get(0)?, received_text: r.get(1)?, code: r.get(2)?, code_system: r.get(3)?, source: r.get(4)?, reporter: r.get(5)?, provided_on: r.get(6)?, as_of_date: r.get(7)?, added_in_version: r.get(8)?, removed_in_version: r.get(9)? }))?.collect::<Result<_, _>>()?;
    let point_of_care_results = conn.prepare_cached("SELECT id, device_or_test, analyte_text, result_text, performed_at_text, source, added_in_version, removed_in_version FROM ord_poc_results WHERE order_id = ?1 ORDER BY added_in_version, id")?
        .query_map([order_id], |r| Ok(PointOfCareResultView { evidence_id: r.get(0)?, device_or_test: r.get(1)?, analyte_text: r.get(2)?, result_text: r.get(3)?, performed_at_text: r.get(4)?, source: r.get(5)?, added_in_version: r.get(6)?, removed_in_version: r.get(7)? }))?.collect::<Result<_, _>>()?;
    Ok(OrderEvidenceView {
        authority,
        diagnoses,
        medications,
        point_of_care_results,
    })
}

pub(super) fn all_holds(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<ClarificationHoldView>, OrderError> {
    Ok(conn.prepare_cached("SELECT id, deficiency, origin, state, note, opened_at, resolved_at, resolution_reason, revision FROM ord_clarification_holds WHERE order_id = ?1 ORDER BY opened_at, id")?
        .query_map([order_id], |r| Ok(ClarificationHoldView { hold_id: r.get(0)?, deficiency: r.get(1)?, origin: r.get(2)?, state: r.get(3)?, note: r.get(4)?, opened_at: r.get(5)?, resolved_at: r.get(6)?, resolution_reason: r.get(7)?, revision: r.get(8)? }))?.collect::<Result<_, _>>()?)
}

pub(super) fn billing_items(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<BillingReviewItemView>, OrderError> {
    Ok(conn.prepare_cached("SELECT id, element, note, opened_at FROM ord_billing_review_items WHERE order_id = ?1 ORDER BY opened_at, id")?
        .query_map([order_id], |r| Ok(BillingReviewItemView { item_id: r.get(0)?, element: r.get(1)?, note: r.get(2)?, opened_at: r.get(3)? }))?.collect::<Result<_, _>>()?)
}

pub(super) fn cancellations(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<CancellationView>, OrderError> {
    Ok(conn.prepare_cached("SELECT c.id, c.item_id, c.requested_by_text, c.reason, c.state, c.decision_reason, c.revision FROM ord_cancellation_requests c JOIN ord_order_items i ON i.id = c.item_id WHERE i.order_id = ?1 ORDER BY c.id")?
        .query_map([order_id], |r| Ok(CancellationView { request_id: r.get(0)?, item_id: r.get(1)?, requested_by: r.get(2)?, reason: r.get(3)?, state: r.get(4)?, decision_reason: r.get(5)?, revision: r.get(6)? }))?.collect::<Result<_, _>>()?)
}

pub(super) fn trigger_assessments(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<TriggerAssessmentView>, OrderError> {
    Ok(conn.prepare_cached("SELECT t.id, t.parent_item_id, t.orderable_id, t.outcome, t.trigger_source, t.trigger_ref, t.created_item_id, t.reason, t.decided_by, t.decided_at FROM ord_trigger_assessments t JOIN ord_order_items i ON i.id = t.parent_item_id WHERE i.order_id = ?1 ORDER BY t.decided_at, t.id")?
        .query_map([order_id], |r| Ok(TriggerAssessmentView { assessment_id: r.get(0)?, parent_item_id: r.get(1)?, orderable_id: r.get(2)?, outcome: r.get(3)?, trigger_source: r.get(4)?, trigger_ref: r.get(5)?, created_item_id: r.get(6)?, reason: r.get(7)?, decided_by: r.get(8)?, decided_by_display: None, decided_at: r.get(9)? }))?.collect::<Result<_, _>>()?)
}

pub(super) fn empty_evidence() -> OrderEvidenceView {
    OrderEvidenceView {
        authority: Vec::new(),
        diagnoses: Vec::new(),
        medications: Vec::new(),
        point_of_care_results: Vec::new(),
    }
}

pub(super) fn allowed_actions(order: &OrderDetail) -> Vec<String> {
    let legacy_role_missing = order.items.iter().any(|item| item.sequence_role.is_none());
    let actions = match (order.status, legacy_role_missing) {
        (crate::OrderStatus::Received, true) => vec!["reject"],
        (crate::OrderStatus::Received, false) => {
            vec!["amend_evidence", "accept", "reject", "place_hold"]
        }
        (crate::OrderStatus::Accepted, _) => {
            vec!["amend_evidence", "place_hold", "request_follow_on"]
        }
        (crate::OrderStatus::Rejected, _) => Vec::new(),
    };
    actions.into_iter().map(str::to_owned).collect()
}
