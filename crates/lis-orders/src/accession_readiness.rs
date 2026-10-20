//! Shared order-side readiness for accession commands and their read model.

use std::collections::BTreeMap;

use lis_catalog::OrderableVersionId;
use rusqlite::{Connection, OptionalExtension};

use crate::{ItemState, OrderError, OrderId, OrderItemId, OrderStatus};

/// One item's order-side eligibility, before checking accession membership (REQ-ORD-002).
#[derive(Debug, Clone)]
pub struct AccessionItemReadiness {
    /// The requested item.
    pub item_id: OrderItemId,
    /// Its ordered catalog version.
    pub orderable_version_id: OrderableVersionId,
    /// Distinct matrices of active frozen components; empty before acceptance.
    pub required_specimen_type_codes: Vec<String>,
    /// Why the order or item is not ready, or `None` when its order-side gates pass.
    pub reason: Option<&'static str>,
}

/// Reads every item's readiness once, including blocked items (REQ-ORD-001/002).
/// Existing accession membership is owned and checked by specimens.
pub fn accession_item_readiness(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<AccessionItemReadiness>, OrderError> {
    let (status, on_hold): (OrderStatus, bool) = conn
        .query_row(
            "SELECT status, EXISTS(SELECT 1 FROM ord_clarification_holds \
             WHERE order_id = o.id AND state = 'open') FROM ord_orders o WHERE id = ?1",
            [order_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))?;
    let mut items = BTreeMap::<OrderItemId, (AccessionItemReadiness, ItemState)>::new();
    let mut statement = conn.prepare_cached(
        "SELECT DISTINCT i.id, i.orderable_version_id, i.state, c.specimen_type_code \
         FROM ord_order_items i LEFT JOIN ord_order_components c \
         ON c.item_id = i.id AND c.state = 'active' \
         WHERE i.order_id = ?1 ORDER BY i.id, c.specimen_type_code",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok((
            row.get::<_, OrderItemId>(0)?,
            row.get::<_, OrderableVersionId>(1)?,
            row.get::<_, ItemState>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        let (item_id, orderable_version_id, state, matrix) = row?;
        let (item, _) = items.entry(item_id).or_insert_with(|| {
            (
                AccessionItemReadiness {
                    item_id,
                    orderable_version_id,
                    required_specimen_type_codes: Vec::new(),
                    reason: None,
                },
                state,
            )
        });
        if let Some(matrix) = matrix {
            item.required_specimen_type_codes.push(matrix);
        }
    }
    Ok(items
        .into_values()
        .map(|(mut item, state)| {
            item.reason = blocking_reason(
                status,
                state,
                on_hold,
                item.required_specimen_type_codes.is_empty(),
            );
            item
        })
        .collect())
}

fn blocking_reason(
    status: OrderStatus,
    state: ItemState,
    on_hold: bool,
    no_active_components: bool,
) -> Option<&'static str> {
    if status != OrderStatus::Accepted {
        return Some("The order has not been accepted.");
    }
    if on_hold {
        return Some("An order clarification hold is open.");
    }
    match state {
        ItemState::Requested => return Some("The requested test has not been accepted."),
        ItemState::Declined => return Some("The requested test was declined."),
        ItemState::Cancelled => return Some("The ordered test was cancelled."),
        ItemState::Accepted => {}
    }
    no_active_components.then_some("The ordered test has no active components.")
}
