//! Accessions, specimens, containers and orders as audit records: the accession number or
//! container label the audit trail shows, and finding them by it (REQ-OPS-001). Labels are
//! resolved from the owner tables at read time and never copied into audit storage.

use lis_audit::{RecordRef, RecordSearch};
use rusqlite::{Connection, OptionalExtension};

use crate::error::SpecimenError;

/// Current accession number of an accession or specimen audit record, label of a
/// container, or accession numbers of an order.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, SpecimenError> {
    let sql = match record_type {
        "accession" => "SELECT accession_number FROM spc_accessions WHERE id = ?1",
        "specimen" => {
            "SELECT accession.accession_number FROM spc_specimens specimen \
             JOIN spc_collections collection ON collection.id = specimen.collection_id \
             JOIN spc_accessions accession ON accession.id = collection.accession_id \
             WHERE specimen.id = ?1"
        }
        "container" => "SELECT label_code FROM spc_containers WHERE id = ?1",
        // NULL, not a row, when the order has no accession yet.
        "order" => {
            "SELECT group_concat(accession_number, ', ') FROM \
             (SELECT accession_number FROM spc_accessions WHERE order_id = ?1 \
             ORDER BY accession_number)"
        }
        _ => return Ok(None),
    };
    Ok(conn
        .prepare_cached(sql)?
        .query_row([record_id], |row| row.get::<_, Option<String>>(0))
        .optional()?
        .flatten())
}

/// Accessions, their specimens and orders by the start of the accession number, and
/// containers by the start of their label or of their accession number.
pub fn audit_record_search(
    conn: &Connection,
    search: &RecordSearch<'_>,
) -> Result<Vec<RecordRef>, SpecimenError> {
    let mut records = search.records(
        conn,
        "accession",
        "SELECT id FROM spc_accessions WHERE accession_number LIKE ?1 ESCAPE '\\' \
         ORDER BY accession_number LIMIT ?2",
    )?;
    records.extend(search.records(
        conn,
        "order",
        "SELECT DISTINCT order_id FROM spc_accessions WHERE accession_number LIKE ?1 \
         ESCAPE '\\' ORDER BY order_id LIMIT ?2",
    )?);
    records.extend(search.records(
        conn,
        "specimen",
        "SELECT specimen.id FROM spc_specimens specimen \
         JOIN spc_collections collection ON collection.id = specimen.collection_id \
         JOIN spc_accessions accession ON accession.id = collection.accession_id \
         WHERE accession.accession_number LIKE ?1 ESCAPE '\\' \
         ORDER BY accession.accession_number, specimen.id LIMIT ?2",
    )?);
    records.extend(search.records(
        conn,
        "container",
        "SELECT container.id FROM spc_containers container \
         JOIN spc_specimens specimen ON specimen.id = container.specimen_id \
         JOIN spc_collections collection ON collection.id = specimen.collection_id \
         JOIN spc_accessions accession ON accession.id = collection.accession_id \
         WHERE container.label_code LIKE ?1 ESCAPE '\\' \
         OR accession.accession_number LIKE ?1 ESCAPE '\\' \
         ORDER BY container.label_code LIMIT ?2",
    )?);
    Ok(records)
}
