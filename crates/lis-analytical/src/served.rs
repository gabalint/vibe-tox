//! Which order components a method version serves for an accession's
//! material (REQ-ANA-001): one home for the worklist, patient positions and
//! the expected-analyte counts of a run.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use lis_catalog::{ComponentView, MethodVersionId, OrderableVersionId, OrderableVersionView};
use lis_orders::OrderComponentId;
use lis_specimens::{AccessionDetail, AccessionId};
use rusqlite::Connection;

use crate::error::AnalyticalError;
use crate::repo::published::{self, ComponentRef};

/// An order component together with its frozen catalog component.
pub(crate) struct ServedComponent {
    pub(crate) component: ComponentRef,
    pub(crate) orderable_code: String,
}

/// Orderable versions read once per call: they are immutable once approved.
#[derive(Default)]
pub(crate) struct CatalogCache(BTreeMap<OrderableVersionId, OrderableVersionView>);

impl CatalogCache {
    fn version(
        &mut self,
        conn: &Connection,
        id: OrderableVersionId,
    ) -> Result<&OrderableVersionView, AnalyticalError> {
        Ok(match self.0.entry(id) {
            Entry::Occupied(found) => found.into_mut(),
            Entry::Vacant(slot) => slot.insert(lis_catalog::get_orderable_version(conn, id)?),
        })
    }

    /// The frozen catalog component of an order component, with its orderable code.
    fn component(
        &mut self,
        conn: &Connection,
        component: &ComponentRef,
    ) -> Result<(String, ComponentView), AnalyticalError> {
        let version = self.version(conn, component.orderable_version_id)?;
        let found = version
            .components
            .iter()
            .find(|catalog| catalog.position == component.position)
            .ok_or(AnalyticalError::NotFound("orderable_component"))?;
        Ok((version.code.clone(), found.clone()))
    }
}

/// The accession and its order components that `method_version_id` serves,
/// active or not. Callers recheck every gate through the owners.
pub(crate) fn served_components(
    conn: &Connection,
    cache: &mut CatalogCache,
    accession_id: AccessionId,
    method_version_id: MethodVersionId,
) -> Result<(AccessionDetail, Vec<ServedComponent>), AnalyticalError> {
    let accession = lis_specimens::get_accession(conn, accession_id)?;
    let mut served = Vec::new();
    for component in published::order_components(conn, accession.order_id)? {
        if !accession.order_item_ids.contains(&component.item_id) {
            continue;
        }
        let (orderable_code, catalog) = cache.component(conn, &component)?;
        if catalog.method_version_id == method_version_id {
            served.push(ServedComponent {
                component,
                orderable_code,
            });
        }
    }
    Ok((accession, served))
}

/// How many analytes the given order components expect in total.
pub(crate) fn expected_analytes(
    conn: &Connection,
    cache: &mut CatalogCache,
    component_ids: &[OrderComponentId],
) -> Result<usize, AnalyticalError> {
    let mut total = 0;
    for id in component_ids {
        let component =
            published::component(conn, *id)?.ok_or(AnalyticalError::NotFound("order_component"))?;
        total += cache.component(conn, &component)?.1.analytes.len();
    }
    Ok(total)
}
