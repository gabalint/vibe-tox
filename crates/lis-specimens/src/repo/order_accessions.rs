//! Bounded-to-one-order membership and receipt progress; no per-item detail reads.

use std::collections::BTreeMap;

use lis_orders::{OrderId, OrderItemId};
use rusqlite::Connection;

use crate::{AccessionId, OrderAccessionSummary};

pub(crate) fn memberships(
    conn: &Connection,
    order_id: OrderId,
) -> rusqlite::Result<BTreeMap<OrderItemId, (AccessionId, String)>> {
    conn.prepare_cached(
        "SELECT i.order_item_id, a.id, a.accession_number FROM spc_accessions a \
         JOIN spc_accession_items i ON i.accession_id = a.id WHERE a.order_id = ?1",
    )?
    .query_map([order_id], |row| {
        Ok((row.get(0)?, (row.get(1)?, row.get(2)?)))
    })?
    .collect()
}

pub(crate) fn summaries(
    conn: &Connection,
    order_id: OrderId,
) -> rusqlite::Result<Vec<OrderAccessionSummary>> {
    conn.prepare_cached(
        "SELECT a.id, a.accession_number, a.created_at, count(s.id), \
         count(CASE WHEN s.acceptance IN ('received', 'held') THEN 1 END) \
         FROM spc_accessions a LEFT JOIN spc_collections c ON c.accession_id = a.id \
         LEFT JOIN spc_specimens s ON s.collection_id = c.id WHERE a.order_id = ?1 \
         GROUP BY a.id ORDER BY a.created_at, a.id",
    )?
    .query_map([order_id], |row| {
        Ok(OrderAccessionSummary {
            accession_id: row.get(0)?,
            accession_number: row.get(1)?,
            created_at: row.get(2)?,
            specimen_count: row.get(3)?,
            pending_receipt_count: row.get(4)?,
        })
    })?
    .collect()
}
