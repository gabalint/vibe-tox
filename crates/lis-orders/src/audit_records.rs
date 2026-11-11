//! Orders as audit records by placer order number (REQ-OPS-001). The accession numbers of
//! an order come from `lis-specimens`, which owns accessions; the placer order number is
//! the label of an order not yet accessioned. Labels are resolved at read time and never
//! copied into audit storage.

use lis_audit::{RecordRef, RecordSearch};
use rusqlite::{Connection, OptionalExtension};

use crate::error::OrderError;

/// The placer order number of an order audit record, when the source supplied one.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, OrderError> {
    if record_type != "order" {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached(
            "SELECT 'Placer order ' || placer_order_id FROM ord_orders \
             WHERE id = ?1 AND placer_order_id IS NOT NULL",
        )?
        .query_row([record_id], |row| row.get(0))
        .optional()?)
}

/// Orders whose placer order number starts with the searched text.
pub fn audit_record_search(
    conn: &Connection,
    search: &RecordSearch<'_>,
) -> Result<Vec<RecordRef>, OrderError> {
    Ok(search.records(
        conn,
        "order",
        "SELECT id FROM ord_orders WHERE placer_order_id LIKE ?1 ESCAPE '\\' \
         ORDER BY placer_order_id LIMIT ?2",
    )?)
}
