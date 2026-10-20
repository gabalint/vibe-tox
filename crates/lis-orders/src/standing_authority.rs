//! Patient-specific standing authority is effective at collection, never intake (REQ-ORD-006).

use lis_core::GateReason;
use rusqlite::{Connection, OptionalExtension};

use crate::{OrderError, OrderId};

/// Rechecks inclusive validity dates against the supplied collection civil date.
/// The specimen owner derives that date from original offset/precision evidence.
/// Acceptance reserves a visit and checks patient, practitioner and ordered scope;
/// an unknown collection date can be retained but cannot authorize testing/release.
pub fn standing_collection_gate(
    conn: &Connection,
    order_id: OrderId,
    collection_date: Option<&str>,
) -> Result<Vec<GateReason>, OrderError> {
    let period: Option<(String, String)> = conn
        .query_row(
            "SELECT s.valid_from, s.valid_until FROM ord_orders o \
             JOIN ord_standing_orders s ON s.id = o.standing_order_id WHERE o.id = ?1",
            [order_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((from, until)) = period else {
        return Ok(Vec::new());
    };
    let Some(date) = collection_date else {
        return Ok(vec![GateReason {
            code: "standing_collection_unknown".into(),
            message:
                "Supply source-backed collection timing before testing under this standing order."
                    .into(),
        }]);
    };
    super::commands::validation::iso_date("collectionDate", date)?;
    if date < from.as_str() || date > until.as_str() {
        return Ok(vec![GateReason {
            code: "standing_collection_outside_validity".into(),
            message: "Collection falls outside the standing order's inclusive validity dates. Resolve the ordering authority; a quality review cannot extend it.".into(),
        }]);
    }
    Ok(Vec::new())
}
