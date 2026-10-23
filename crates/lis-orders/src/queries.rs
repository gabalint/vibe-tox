//! Published read contracts. Authorization and sensitive-read auditing are
//! performed by the engine; safety gates call these inside the write transaction.

use lis_catalog::{ComponentPurpose, OrderableVersionId};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use lis_core::{GateReason, UtcTimestamp};
use lis_identity::{ClientId, SubjectId, SubjectSummary};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use ts_rs::TS;

use crate::error::OrderError;
use crate::inputs::ListOrdersInput;
use crate::model::{OrderComponentId, OrderId, OrderItemId, OrderStatus};
use crate::query_detail::{
    all_holds, allowed_actions, billing_items, cancellations, empty_evidence, evidence,
    order_items, trigger_assessments,
};
use crate::views::{ClarificationHoldView, OrderDetail, OrderSummary, StandingOrderView};

/// An accepted order item that may be linked to a new accession.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemReadyForAccession {
    pub item_id: OrderItemId,
    pub orderable_version_id: OrderableVersionId,
    pub specimen_type_code: String,
}

/// Returns accepted, uncancelled items whose order has no open clarification
/// hold, with the frozen orderable version and specimen matrix (REQ-ORD-001/002).
pub fn items_ready_for_accession(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<ItemReadyForAccession>, OrderError> {
    Ok(crate::accession_item_readiness(conn, order_id)?
        .into_iter()
        .filter(|item| item.reason.is_none())
        .flat_map(|item| {
            item.required_specimen_type_codes
                .into_iter()
                .map(move |matrix| ItemReadyForAccession {
                    item_id: item.item_id,
                    orderable_version_id: item.orderable_version_id,
                    specimen_type_code: matrix,
                })
        })
        .collect())
}

/// The component state required before analytical testing begins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComponentForTesting {
    pub component_id: OrderComponentId,
    pub order_id: OrderId,
    pub subject_id: SubjectId,
    pub item_id: OrderItemId,
    pub orderable_version_id: OrderableVersionId,
    #[ts(type = "number")]
    pub catalog_component_position: i64,
    pub specimen_type_code: String,
    pub purpose: ComponentPurpose,
    pub sequence_role: crate::SequenceRole,
}

/// Rechecks every order-side gate for analytical testing (REQ-ORD-002).
pub fn testing_gate(
    conn: &Connection,
    component_id: OrderComponentId,
) -> Result<Result<ComponentForTesting, Vec<GateReason>>, OrderError> {
    let row = conn
        .query_row(
            "SELECT component_id, order_id, subject_id, item_id, catalog_orderable_version_id, \
             catalog_component_position, specimen_type_code, purpose, sequence_role, order_status, \
             item_state, component_state, open_hold_count FROM ord_v_components \
             WHERE component_id = ?1",
            [component_id],
            |row| {
                Ok((
                    component_from_row(row)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, i64>(12)?,
                ))
            },
        )
        .optional()?
        .ok_or(OrderError::NotFound("order_component"))?;
    let mut gates = Vec::new();
    if row.1 != "accepted" {
        gates.push(gate(
            "order_not_accepted",
            "The order has not been accepted.",
        ));
    }
    if row.4 > 0 {
        gates.push(gate(
            "clarification_hold_open",
            "An order clarification hold is open.",
        ));
    }
    if row.3 != "active" {
        gates.push(gate(
            "component_cancelled",
            "The ordered component was cancelled.",
        ));
    }
    if row.2 != "accepted" {
        gates.push(gate("item_not_accepted", "The order item is not accepted."));
    }
    if gates.is_empty() {
        Ok(Ok(row.0))
    } else {
        Ok(Err(gates))
    }
}

/// Frozen components of an order, including cancelled components.
pub fn components_for_order(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<ComponentForTesting>, OrderError> {
    ensure_order_exists(conn, order_id)?;
    Ok(conn
        .prepare_cached(
            "SELECT component_id, order_id, subject_id, item_id, catalog_orderable_version_id, \
             catalog_component_position, specimen_type_code, purpose, sequence_role \
             FROM ord_v_components WHERE order_id = ?1 ORDER BY item_id, catalog_component_position",
        )?
        .query_map([order_id], |row| {
            component_from_row(row)
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

/// Full order detail. The engine treats it as an audited sensitive read.
pub fn get_order(conn: &Connection, order_id: OrderId) -> Result<OrderDetail, OrderError> {
    let mut detail = conn
        .query_row(
            "SELECT id, version, revision, status, subject_id, practitioner_id, client_id, \
             client_location_id, placer_order_id, placer_authority, source_kind, source_document_ref, \
             billing_responsibility, facility_setting, standing_order_id, template_name, \
             template_confirmed, received_at FROM ord_orders WHERE id = ?1",
            [order_id],
            |row| {
                Ok(OrderDetail {
                    order_id: row.get(0)?,
                    version: row.get(1)?,
                    revision: row.get(2)?,
                    status: row.get(3)?,
                    subject_id: row.get(4)?,
                    practitioner_id: row.get(5)?,
                    client_id: row.get(6)?,
                    client_location_id: row.get(7)?,
                    placer_order_id: row.get(8)?,
                    placer_authority: row.get(9)?,
                    source_kind: row.get(10)?,
                    source_document_ref: row.get(11)?,
                    billing_responsibility: row.get(12)?,
                    facility_setting: row.get(13)?,
                    standing_order_id: row.get(14)?,
                    template_name: row.get(15)?,
                    template_confirmed_per_patient: row.get(16)?,
                    received_at: row.get(17)?,
                    items: Vec::new(),
                    evidence: empty_evidence(),
                    holds: Vec::new(),
                    billing_review_items: Vec::new(),
                    cancellations: Vec::new(),
                    trigger_assessments: Vec::new(),
                    allowed_actions: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))?;
    detail.items = order_items(conn, order_id)?;
    detail.evidence = evidence(conn, order_id)?;
    detail.holds = all_holds(conn, order_id)?;
    detail.billing_review_items = billing_items(conn, order_id)?;
    detail.cancellations = cancellations(conn, order_id)?;
    detail.trigger_assessments = trigger_assessments(conn, order_id)?;
    detail.allowed_actions = allowed_actions(&detail);
    Ok(detail)
}

/// Bounded order worklist, newest first, with each order's patient name, placer order
/// number, client name and item count (REQ-ORD-001). PHI: the engine audits the read.
pub fn list_orders(
    conn: &Connection,
    input: &ListOrdersInput,
) -> Result<Vec<OrderSummary>, OrderError> {
    if input.limit == 0 || input.limit > 500 {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Use a limit from 1 to 500.",
        ));
    }
    let filter = match input.filter {
        crate::OrderFilter::All => "1",
        crate::OrderFilter::NeedsDecision => "o.status = 'received'",
        crate::OrderFilter::OnHold => {
            "EXISTS (SELECT 1 FROM ord_clarification_holds h \
            WHERE h.order_id = o.id AND h.state = 'open')"
        }
        crate::OrderFilter::Accepted => "o.status = 'accepted'",
    };
    let subject = if input.subject_id.is_some() {
        "o.subject_id = ?1"
    } else {
        "1"
    };
    if let Some(id) = input.before_order_id {
        ensure_order_exists(conn, id)?;
    }
    let before = "(?4 IS NULL OR (o.received_at, o.id) < (SELECT received_at, id FROM ord_orders WHERE id=?4)) AND (?4 IS NOT NULL OR ?2 IS NULL OR o.received_at < ?2)";
    let sql = format!(
        "WITH page AS MATERIALIZED (SELECT o.id, o.subject_id, o.client_id, o.placer_order_id, \
         o.status, o.received_at, o.version, o.revision FROM ord_orders o \
         WHERE {filter} AND {subject} AND {before} \
         ORDER BY o.received_at DESC, o.id DESC LIMIT ?3) \
         SELECT o.id, o.subject_id, o.client_id, o.placer_order_id, o.status, o.received_at, \
         o.version, o.revision, \
         (SELECT count(*) FROM ord_clarification_holds h WHERE h.order_id = o.id AND h.state = 'open'), \
         (SELECT count(*) FROM ord_order_items i WHERE i.order_id = o.id) \
         FROM page o ORDER BY o.received_at DESC, o.id DESC"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(
            params![
                input.subject_id,
                input.before_received_at,
                input.limit,
                input.before_order_id
            ],
            |row| {
                Ok(OrderRow {
                    order_id: row.get(0)?,
                    subject_id: row.get(1)?,
                    client_id: row.get(2)?,
                    placer_order_id: row.get(3)?,
                    status: row.get(4)?,
                    received_at: row.get(5)?,
                    version: row.get(6)?,
                    revision: row.get(7)?,
                    open_hold_count: row.get(8)?,
                    item_count: row.get(9)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    with_names(conn, rows)
}

/// One row of the order worklist query, before the names are read.
struct OrderRow {
    order_id: OrderId,
    subject_id: SubjectId,
    client_id: ClientId,
    placer_order_id: Option<String>,
    status: OrderStatus,
    received_at: UtcTimestamp,
    version: i64,
    revision: i64,
    open_hold_count: i64,
    item_count: i64,
}

/// Adds the patient and client names, read through the identity module's published
/// queries (never from its tables), once per distinct patient and client.
fn with_names(conn: &Connection, rows: Vec<OrderRow>) -> Result<Vec<OrderSummary>, OrderError> {
    let mut subjects: BTreeMap<SubjectId, SubjectSummary> = BTreeMap::new();
    let mut clients: BTreeMap<ClientId, String> = BTreeMap::new();
    rows.into_iter()
        .map(|row| {
            let subject = match subjects.entry(row.subject_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    entry.insert(lis_identity::subject_summary(conn, row.subject_id)?)
                }
            };
            let client_name = match clients.entry(row.client_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    entry.insert(lis_identity::get_client(conn, row.client_id)?.name)
                }
            };
            Ok(OrderSummary {
                order_id: row.order_id,
                subject_id: row.subject_id,
                patient_family_name: subject.family_name.clone(),
                patient_given_names: subject.given_names.clone(),
                patient_birth_date: subject.birth_date.clone(),
                patient_identifiers: subject.identifiers.clone(),
                placer_order_id: row.placer_order_id,
                client_name: client_name.clone(),
                sample_barcodes: Vec::new(),
                item_count: row.item_count,
                status: row.status,
                received_at: row.received_at,
                version: row.version,
                revision: row.revision,
                open_hold_count: row.open_hold_count,
            })
        })
        .collect()
}

/// Every open hold for an order.
pub fn open_holds(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<ClarificationHoldView>, OrderError> {
    ensure_order_exists(conn, order_id)?;
    Ok(all_holds(conn, order_id)?
        .into_iter()
        .filter(|hold| hold.state == "open")
        .collect())
}

/// Patient-specific standing orders, newest first.
pub fn list_standing_orders(
    conn: &Connection,
    subject_id: SubjectId,
) -> Result<Vec<StandingOrderView>, OrderError> {
    let mut rows: Vec<StandingOrderView> = conn
        .prepare_cached(
            "SELECT id, subject_id, practitioner_id, valid_from, valid_until, visit_limit, \
             document_ref, recorded_at, (SELECT count(*) FROM ord_orders o \
             WHERE o.standing_order_id = ord_standing_orders.id AND o.status = 'accepted') \
             FROM ord_standing_orders WHERE subject_id = ?1 \
             ORDER BY recorded_at DESC, id DESC",
        )?
        .query_map([subject_id], |row| {
            Ok(StandingOrderView {
                accepted_uses: row.get(8)?,
                standing_order_id: row.get(0)?,
                subject_id: row.get(1)?,
                practitioner_id: row.get(2)?,
                valid_from: row.get(3)?,
                valid_until: row.get(4)?,
                visit_limit: row.get(5)?,
                document_ref: row.get(6)?,
                recorded_at: row.get(7)?,
                scope_orderable_ids: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    for row in &mut rows {
        row.scope_orderable_ids = conn
            .prepare_cached(
                "SELECT orderable_id FROM ord_standing_order_scope \
                 WHERE standing_order_id = ?1 ORDER BY orderable_id",
            )?
            .query_map([row.standing_order_id], |record| record.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(rows)
}

fn ensure_order_exists(conn: &Connection, order_id: OrderId) -> Result<(), OrderError> {
    conn.query_row("SELECT 1 FROM ord_orders WHERE id = ?1", [order_id], |_| {
        Ok(())
    })
    .optional()?
    .ok_or(OrderError::NotFound("order"))
}

fn gate(code: &str, message: &str) -> GateReason {
    GateReason {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn component_from_row(row: &Row<'_>) -> rusqlite::Result<ComponentForTesting> {
    Ok(ComponentForTesting {
        component_id: row.get(0)?,
        order_id: row.get(1)?,
        subject_id: row.get(2)?,
        item_id: row.get(3)?,
        orderable_version_id: row.get(4)?,
        catalog_component_position: row.get(5)?,
        specimen_type_code: row.get(6)?,
        purpose: row.get(7)?,
        sequence_role: row.get(8)?,
    })
}
