//! Reads of other modules' published views (`spc_v_containers`,
//! `ord_v_components`). Gates are never decided here: callers recheck them
//! through `lis_specimens::testing_gate` and `lis_orders::testing_gate`.

use lis_catalog::OrderableVersionId;
use lis_orders::{ItemKind, OrderComponentId, OrderId, OrderItemId};
use lis_specimens::{AccessionId, ContainerId, SpecimenId};
use rusqlite::{Connection, OptionalExtension, Row, params};

/// A container as published by lis-specimens.
pub(crate) struct ContainerRef {
    pub(crate) container_id: ContainerId,
    pub(crate) label_code: String,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) accession_id: AccessionId,
    pub(crate) accession_number: String,
}

const CONTAINER_COLUMNS: &str =
    "container_id, label_code, specimen_id, accession_id, accession_number";

fn container_from_row(row: &Row<'_>) -> rusqlite::Result<ContainerRef> {
    Ok(ContainerRef {
        container_id: row.get(0)?,
        label_code: row.get(1)?,
        specimen_id: row.get(2)?,
        accession_id: row.get(3)?,
        accession_number: row.get(4)?,
    })
}

/// The container with this printed label code.
pub(crate) fn container_by_label(
    conn: &Connection,
    label_code: &str,
) -> rusqlite::Result<Option<ContainerRef>> {
    conn.prepare_cached(&format!(
        "SELECT {CONTAINER_COLUMNS} FROM spc_v_containers WHERE label_code = ?1"
    ))?
    .query_row([label_code], container_from_row)
    .optional()
}

/// The container by ID.
pub(crate) fn container(
    conn: &Connection,
    container_id: ContainerId,
) -> rusqlite::Result<Option<ContainerRef>> {
    conn.prepare_cached(&format!(
        "SELECT {CONTAINER_COLUMNS} FROM spc_v_containers WHERE container_id = ?1"
    ))?
    .query_row([container_id], container_from_row)
    .optional()
}

/// Containers that may hold pending work for the method: available, accepted
/// (or conditionally), without an open hold or active execution. The worklist
/// still rechecks each through the owners' gates.
pub(crate) fn candidate_containers(
    conn: &Connection,
    specimen_type_code: &str,
    method_version_id: lis_catalog::MethodVersionId,
) -> rusqlite::Result<Vec<ContainerRef>> {
    conn.prepare_cached(&format!(
        "SELECT {CONTAINER_COLUMNS} FROM spc_v_containers WHERE specimen_type_code = ?1 \
         AND state = 'available' AND acceptance IN ('accepted', 'accepted_conditionally') \
         AND open_hold_count = 0 AND NOT EXISTS (SELECT 1 FROM ana_executions execution \
           WHERE execution.container_id = spc_v_containers.container_id \
           AND execution.method_version_id = ?2 AND execution.state = 'active') \
         ORDER BY accession_number, label_code"
    ))?
    .query_map(
        params![specimen_type_code, method_version_id],
        container_from_row,
    )?
    .collect()
}

/// An order component as published by lis-orders.
pub(crate) struct ComponentRef {
    pub(crate) component_id: OrderComponentId,
    pub(crate) item_id: OrderItemId,
    pub(crate) item_kind: ItemKind,
    /// The item is accepted and the component is active.
    pub(crate) active: bool,
    pub(crate) orderable_version_id: OrderableVersionId,
    pub(crate) position: i64,
}

const COMPONENT_COLUMNS: &str = "component_id, item_id, item_kind, \
     item_state = 'accepted' AND component_state = 'active', catalog_orderable_version_id, \
     catalog_component_position";

fn component_from_row(row: &Row<'_>) -> rusqlite::Result<ComponentRef> {
    Ok(ComponentRef {
        component_id: row.get(0)?,
        item_id: row.get(1)?,
        item_kind: row.get(2)?,
        active: row.get(3)?,
        orderable_version_id: row.get(4)?,
        position: row.get(5)?,
    })
}

/// Every component of an order.
pub(crate) fn order_components(
    conn: &Connection,
    order_id: OrderId,
) -> rusqlite::Result<Vec<ComponentRef>> {
    conn.prepare_cached(&format!(
        "SELECT {COMPONENT_COLUMNS} FROM ord_v_components WHERE order_id = ?1 \
         ORDER BY item_id, catalog_component_position"
    ))?
    .query_map(params![order_id], component_from_row)?
    .collect()
}

/// One component.
pub(crate) fn component(
    conn: &Connection,
    component_id: OrderComponentId,
) -> rusqlite::Result<Option<ComponentRef>> {
    conn.prepare_cached(&format!(
        "SELECT {COMPONENT_COLUMNS} FROM ord_v_components WHERE component_id = ?1"
    ))?
    .query_row([component_id], component_from_row)
    .optional()
}
