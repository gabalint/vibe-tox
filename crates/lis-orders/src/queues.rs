//! PHI-free order work queues for operational composition (REQ-OPS-004).
//!
//! Queue rows expose only order identifiers, supplied order numbers, state
//! codes and timestamps. Results are bounded and oldest first.

use lis_core::UtcTimestamp;
use rusqlite::Connection;

use crate::{OrderError, OrderId};

// Engine consumers bound external pages; owner composition may request all pending tasks.

/// One order awaiting operational action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderQueueItem {
    /// The order to open.
    pub order_id: OrderId,
    /// The supplied order number, or the LIS order ID when none was supplied.
    pub order_number: String,
    /// A PHI-free order or hold state code.
    pub state: String,
    /// When the item entered this queue.
    pub since: UtcTimestamp,
}

/// Received orders that still need an accept, hold or reject decision.
pub fn orders_awaiting_decision(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<OrderQueueItem>, OrderError> {
    Ok(conn
        .prepare_cached(
            "SELECT id, coalesce(placer_order_id, id), status, received_at \
             FROM ord_orders WHERE status = 'received' \
             ORDER BY received_at, id LIMIT ?1",
        )?
        .query_map([bounded(limit)], queue_row)?
        .collect::<Result<_, _>>()?)
}

/// Orders with an open clarification hold, by the oldest open hold.
pub fn orders_with_open_hold(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<OrderQueueItem>, OrderError> {
    Ok(conn
        .prepare_cached(
            "SELECT o.id, coalesce(o.placer_order_id, o.id), \
             (SELECT first.deficiency FROM ord_clarification_holds first \
              WHERE first.order_id = o.id AND first.state = 'open' \
              ORDER BY first.opened_at, first.id LIMIT 1), min(h.opened_at) \
             FROM ord_orders o JOIN ord_clarification_holds h ON h.order_id = o.id \
             WHERE h.state = 'open' GROUP BY o.id \
             ORDER BY min(h.opened_at), o.id LIMIT ?1",
        )?
        .query_map([bounded(limit)], queue_row)?
        .collect::<Result<_, _>>()?)
}

fn bounded(limit: u32) -> u32 {
    limit.max(1)
}

fn queue_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OrderQueueItem> {
    Ok(OrderQueueItem {
        order_id: row.get(0)?,
        order_number: row.get(1)?,
        state: row.get(2)?,
        since: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn connection() -> rusqlite::Result<Connection> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE ord_orders (
                id TEXT PRIMARY KEY, placer_order_id TEXT, status TEXT, received_at TEXT
             ) STRICT;
             CREATE TABLE ord_clarification_holds (
                id TEXT PRIMARY KEY, order_id TEXT, deficiency TEXT, state TEXT, opened_at TEXT
             ) STRICT;",
        )?;
        Ok(conn)
    }

    fn insert_order(
        conn: &Connection,
        id: OrderId,
        number: Option<&str>,
        status: &str,
        received_at: &str,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO ord_orders VALUES (?1, ?2, ?3, ?4)",
            params![id, number, status, received_at],
        )?;
        Ok(())
    }

    #[test]
    fn decision_queue_is_bounded_and_oldest_first() -> Result<(), Box<dyn std::error::Error>> {
        let conn = connection()?;
        let old = OrderId::generate();
        let new = OrderId::generate();
        insert_order(
            &conn,
            new,
            Some("SYN-NEW"),
            "received",
            "2026-10-13T11:00:00.000000000Z",
        )?;
        insert_order(
            &conn,
            old,
            Some("SYN-OLD"),
            "received",
            "2026-10-13T10:00:00.000000000Z",
        )?;
        insert_order(
            &conn,
            OrderId::generate(),
            Some("SYN-DONE"),
            "accepted",
            "2026-10-13T09:00:00.000000000Z",
        )?;

        let items = orders_awaiting_decision(&conn, 1)?;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].order_id, old);
        assert_eq!(items[0].state, "received");
        Ok(())
    }

    #[test]
    fn hold_queue_uses_first_open_hold_and_excludes_resolved()
    -> Result<(), Box<dyn std::error::Error>> {
        let conn = connection()?;
        let order = OrderId::generate();
        insert_order(
            &conn,
            order,
            None,
            "accepted",
            "2026-10-13T08:00:00.000000000Z",
        )?;
        for (deficiency, state, opened_at) in [
            (
                "signature_or_intent_required",
                "open",
                "2026-10-13T10:00:00.000000000Z",
            ),
            ("other_authority", "open", "2026-10-13T11:00:00.000000000Z"),
            (
                "requestor_missing",
                "resolved",
                "2026-10-13T09:00:00.000000000Z",
            ),
        ] {
            conn.execute(
                "INSERT INTO ord_clarification_holds VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    crate::ClarificationHoldId::generate(),
                    order,
                    deficiency,
                    state,
                    opened_at
                ],
            )?;
        }

        let items = orders_with_open_hold(&conn, 10)?;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].order_number, order.to_string());
        assert_eq!(items[0].state, "signature_or_intent_required");
        assert_eq!(items[0].since.to_string(), "2026-10-13T10:00:00.000000000Z");
        Ok(())
    }
}
