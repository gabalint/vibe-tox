//! The expected-component manifest of a specimen, the one home of that rule
//! (brief §3.2, REQ-RPT-005), published to billing. Expected are the
//! components of the accession's items, plus reflex items whose parent is
//! one of them, whose matrix equals the specimen's.

use std::collections::BTreeSet;

use lis_catalog::{ComponentView, OrderableId, OrderableVersionId};
use lis_orders::{
    ComponentState, ItemKind, ItemState, OrderDetail, OrderId, OrderItemId, OrderItemView,
};
use lis_specimens::{AccessionDetail, AccessionId, SpecimenId};
use rusqlite::Connection;

use crate::error::ReportingError;

/// What a specimen's report is expected to cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The specimen.
    pub specimen_id: SpecimenId,
    /// Its accession.
    pub accession_id: AccessionId,
    /// The order the accession fulfils.
    pub order_id: OrderId,
    /// Expected components, by item and catalog position.
    pub components: Vec<ExpectedComponent>,
    /// Reflex orderables in scope of an expected item, with any assessment.
    pub triggers: Vec<ExpectedTrigger>,
    /// Expected items also linked to another accession: the gate
    /// `item_in_multiple_accessions` (L1, Q-08).
    pub shared_items: Vec<OrderItemId>,
}

/// One expected component with its frozen catalog definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedComponent {
    /// The order component.
    pub component_id: lis_orders::OrderComponentId,
    /// Its item.
    pub item_id: OrderItemId,
    /// Initial, add-on or reflex.
    pub item_kind: ItemKind,
    /// The item's state.
    pub item_state: ItemState,
    /// The component's state.
    pub state: ComponentState,
    /// The sequence role, as ordered.
    pub sequence_role: lis_orders::SequenceRole,
    /// The orderable.
    pub orderable_id: OrderableId,
    /// The orderable's name in the frozen version.
    pub orderable_name: String,
    /// The frozen orderable version.
    pub orderable_version_id: OrderableVersionId,
    /// The catalog component: purpose, method version, certainty, scale, analytes.
    pub catalog: ComponentView,
}

/// A reflex orderable in an expected item's scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedTrigger {
    /// The item whose scope names it.
    pub parent_item_id: OrderItemId,
    /// The reflex orderable.
    pub orderable_id: OrderableId,
    /// `activated` or `condition_not_met`; `None` while unassessed.
    pub outcome: Option<String>,
}

/// The manifest of `specimen_id`, read with owner queries; usable inside a
/// transaction.
pub fn manifest(conn: &Connection, specimen_id: SpecimenId) -> Result<Manifest, ReportingError> {
    let accession_id = lis_specimens::specimen_accession(conn, specimen_id)?;
    let accession = lis_specimens::get_accession(conn, accession_id)?;
    let order = lis_orders::get_order(conn, accession.order_id)?;
    manifest_of(conn, &accession, specimen_id, &order)
}

/// The manifest from already-read accession and order records.
pub(crate) fn manifest_of(
    conn: &Connection,
    accession: &AccessionDetail,
    specimen_id: SpecimenId,
    order: &OrderDetail,
) -> Result<Manifest, ReportingError> {
    let matrix = &accession
        .specimens
        .iter()
        .find(|specimen| specimen.specimen_id == specimen_id)
        .ok_or(ReportingError::NotFound("specimen"))?
        .specimen_type_code;
    let items = expected_items(accession, order);
    let mut components = Vec::new();
    let mut matched_items = Vec::new();
    for item in &items {
        let version = lis_catalog::get_orderable_version(conn, item.orderable_version_id)?;
        for component in item
            .components
            .iter()
            .filter(|c| &c.specimen_type_code == matrix)
        {
            let catalog = version
                .components
                .iter()
                .find(|c| c.position == component.catalog_component_position)
                .ok_or(ReportingError::Inconsistent("catalog component"))?;
            components.push(ExpectedComponent {
                component_id: component.component_id,
                item_id: item.item_id,
                item_kind: item.kind,
                item_state: item.state,
                state: component.state,
                sequence_role: component.sequence_role,
                orderable_id: version.orderable_id,
                orderable_name: version.name.clone(),
                orderable_version_id: version.id,
                catalog: catalog.clone(),
            });
            if !matched_items.contains(&item.item_id) {
                matched_items.push(item.item_id);
            }
        }
    }
    Ok(Manifest {
        specimen_id,
        accession_id: accession.accession_id,
        order_id: order.order_id,
        components,
        triggers: triggers(order, &items),
        shared_items: shared_items(conn, accession, &matched_items)?,
    })
}

/// The accession's items, then reflex items whose parent is one of them.
fn expected_items<'a>(
    accession: &AccessionDetail,
    order: &'a OrderDetail,
) -> Vec<&'a OrderItemView> {
    let mut items: Vec<&OrderItemView> = order
        .items
        .iter()
        .filter(|item| accession.order_item_ids.contains(&item.item_id))
        .collect();
    let reflexes: Vec<&OrderItemView> = order
        .items
        .iter()
        .filter(|item| {
            item.kind == ItemKind::Reflex
                && item
                    .parent_item_id
                    .is_some_and(|parent| accession.order_item_ids.contains(&parent))
        })
        .collect();
    items.extend(reflexes);
    items
}

/// Reflex scope of the expected items, with the assessment, if any.
fn triggers(order: &OrderDetail, items: &[&OrderItemView]) -> Vec<ExpectedTrigger> {
    let mut triggers = Vec::new();
    for item in items {
        for orderable_id in &item.reflex_scope_orderable_ids {
            let outcome = order
                .trigger_assessments
                .iter()
                .find(|a| a.parent_item_id == item.item_id && a.orderable_id == *orderable_id)
                .map(|a| a.outcome.clone());
            triggers.push(ExpectedTrigger {
                parent_item_id: item.item_id,
                orderable_id: *orderable_id,
                outcome,
            });
        }
    }
    triggers
}

/// Items in `items` that another accession of the same order also lists.
fn shared_items(
    conn: &Connection,
    accession: &AccessionDetail,
    items: &[OrderItemId],
) -> Result<Vec<OrderItemId>, ReportingError> {
    let others: BTreeSet<AccessionId> =
        lis_specimens::specimens_for_subject(conn, accession.subject_id)?
            .into_iter()
            .filter(|s| {
                s.order_id == accession.order_id && s.accession_id != accession.accession_id
            })
            .map(|s| s.accession_id)
            .collect();
    let mut shared = Vec::new();
    for other in others {
        let detail = lis_specimens::get_accession(conn, other)?;
        for item in items {
            if detail.order_item_ids.contains(item) && !shared.contains(item) {
                shared.push(*item);
            }
        }
    }
    Ok(shared)
}
