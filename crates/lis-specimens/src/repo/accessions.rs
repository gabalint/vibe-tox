//! SQL for accessions, collections, observations, specimens and receipt decisions.

use lis_core::UtcTimestamp;
use lis_identity::SubjectId;
use lis_orders::{OrderId, OrderItemId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::input::CollectionTime;
use crate::model::{
    AccessionId, CollectionId, CollectionObservationId, ReceiptDecision, ReceiptDecisionId,
    ReceiptDiscrepancyCode, SpecimenAcceptance, SpecimenId,
};
use crate::views::{CollectionObservationView, CollectionView, SubjectSpecimen};

/// The subject of an order, read through the orders module's published view.
pub(crate) fn order_subject(
    conn: &Connection,
    order_id: OrderId,
) -> rusqlite::Result<Option<SubjectId>> {
    conn.prepare_cached("SELECT DISTINCT subject_id FROM ord_v_components WHERE order_id = ?1")?
        .query_row([order_id], |row| row.get(0))
        .optional()
}

/// Whether the order item is already linked to an accession.
pub(crate) fn item_accessioned(conn: &Connection, item_id: OrderItemId) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM spc_accession_items WHERE order_item_id = ?1)",
    )?
    .query_row([item_id], |row| row.get(0))
}

pub(crate) struct NewAccession<'a> {
    pub(crate) id: AccessionId,
    pub(crate) number: &'a str,
    pub(crate) order_id: OrderId,
    pub(crate) subject_id: SubjectId,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_accession(conn: &Connection, new: &NewAccession<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_accessions (id, accession_number, order_id, subject_id, created_by, \
         created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        new.id,
        new.number,
        new.order_id,
        new.subject_id,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn insert_accession_item(
    conn: &Connection,
    accession_id: AccessionId,
    item_id: OrderItemId,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_accession_items (accession_id, order_item_id) VALUES (?1, ?2)",
    )?
    .execute(params![accession_id, item_id])?;
    Ok(())
}

pub(crate) struct NewCollection<'a> {
    pub(crate) id: CollectionId,
    pub(crate) accession_id: AccessionId,
    pub(crate) time: &'a CollectionTime,
    pub(crate) collector: Option<&'a str>,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_collection(
    conn: &Connection,
    new: &NewCollection<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_collections (id, accession_id, precision, collected_at, \
         collected_offset_minutes, collected_date, collector_text, recorded_by, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        new.id,
        new.accession_id,
        new.time.precision,
        new.time.collected_at,
        new.time.offset_minutes,
        new.time.date,
        new.collector,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) struct NewObservation<'a> {
    pub(crate) collection_id: CollectionId,
    pub(crate) code: &'a str,
    pub(crate) value_text: &'a str,
    pub(crate) observed_by: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_observation(
    conn: &Connection,
    new: &NewObservation<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_collection_observations (id, collection_id, code, value_text, \
         observed_by_text, recorded_by, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        CollectionObservationId::generate(),
        new.collection_id,
        new.code,
        new.value_text,
        new.observed_by,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn insert_specimen(
    conn: &Connection,
    id: SpecimenId,
    collection_id: CollectionId,
    specimen_type_code: &str,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_specimens (id, collection_id, specimen_type_code, acceptance, \
         received_at, received_by, revision) VALUES (?1, ?2, ?3, 'received', ?4, ?5, 1)",
    )?
    .execute(params![id, collection_id, specimen_type_code, at, by])?;
    Ok(())
}

/// A specimen's acceptance and revision.
pub(crate) fn specimen_state(
    conn: &Connection,
    id: SpecimenId,
) -> rusqlite::Result<Option<(SpecimenAcceptance, i64)>> {
    conn.prepare_cached("SELECT acceptance, revision FROM spc_specimens WHERE id = ?1")?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
}

pub(crate) fn update_acceptance(
    conn: &Connection,
    id: SpecimenId,
    acceptance: SpecimenAcceptance,
    revision: i64,
) -> rusqlite::Result<()> {
    conn.prepare_cached("UPDATE spc_specimens SET acceptance = ?2, revision = ?3 WHERE id = ?1")?
        .execute(params![id, acceptance, revision])?;
    Ok(())
}

pub(crate) struct NewDecision<'a> {
    pub(crate) id: ReceiptDecisionId,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) decision: ReceiptDecision,
    pub(crate) policy_reference: Option<&'a str>,
    pub(crate) scope: Option<&'a str>,
    pub(crate) reason: Option<&'a str>,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_decision(conn: &Connection, new: &NewDecision<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_receipt_decisions (id, specimen_id, decision, policy_reference, \
         scope_text, reason, decided_by, decided_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        new.id,
        new.specimen_id,
        new.decision,
        new.policy_reference,
        new.scope,
        new.reason,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn insert_discrepancy(
    conn: &Connection,
    decision_id: ReceiptDecisionId,
    code: ReceiptDiscrepancyCode,
    note: Option<&str>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_receipt_discrepancies (decision_id, code, note) VALUES (?1, ?2, ?3)",
    )?
    .execute(params![decision_id, code, note])?;
    Ok(())
}

/// The accession row: number, order, subject and creation time.
pub(crate) struct AccessionRow {
    pub(crate) number: String,
    pub(crate) order_id: OrderId,
    pub(crate) subject_id: SubjectId,
    pub(crate) created_at: UtcTimestamp,
}

pub(crate) fn accession(
    conn: &Connection,
    id: AccessionId,
) -> rusqlite::Result<Option<AccessionRow>> {
    conn.prepare_cached(
        "SELECT accession_number, order_id, subject_id, created_at FROM spc_accessions \
         WHERE id = ?1",
    )?
    .query_row([id], |row| {
        Ok(AccessionRow {
            number: row.get(0)?,
            order_id: row.get(1)?,
            subject_id: row.get(2)?,
            created_at: row.get(3)?,
        })
    })
    .optional()
}

pub(crate) fn accession_items(
    conn: &Connection,
    id: AccessionId,
) -> rusqlite::Result<Vec<OrderItemId>> {
    conn.prepare_cached(
        "SELECT order_item_id FROM spc_accession_items WHERE accession_id = ?1 \
         ORDER BY order_item_id",
    )?
    .query_map([id], |row| row.get(0))?
    .collect()
}

/// The accession's collection, without observations.
pub(crate) fn collection(conn: &Connection, id: AccessionId) -> rusqlite::Result<CollectionView> {
    conn.prepare_cached(
        "SELECT id, precision, collected_at, collected_offset_minutes, collected_date, \
         collector_text FROM spc_v_collections WHERE accession_id = ?1",
    )?
    .query_row([id], |row| {
        Ok(CollectionView {
            collection_id: row.get(0)?,
            precision: row.get(1)?,
            collected_at: row.get(2)?,
            utc_offset_minutes: row.get(3)?,
            collected_date: row.get(4)?,
            collector: row.get(5)?,
            observations: Vec::new(),
        })
    })
}

pub(crate) fn observations(
    conn: &Connection,
    collection_id: CollectionId,
) -> rusqlite::Result<Vec<CollectionObservationView>> {
    conn.prepare_cached(
        "SELECT id, code, value_text, observed_by_text FROM spc_collection_observations \
         WHERE collection_id = ?1 ORDER BY id",
    )?
    .query_map([collection_id], |row| {
        Ok(CollectionObservationView {
            observation_id: row.get(0)?,
            code: row.get(1)?,
            value_text: row.get(2)?,
            observed_by: row.get(3)?,
        })
    })?
    .collect()
}

/// Specimen rows of a collection: ID, matrix, acceptance, receipt time, revision.
pub(crate) type SpecimenRow = (SpecimenId, String, SpecimenAcceptance, UtcTimestamp, i64);

pub(crate) fn specimens(
    conn: &Connection,
    collection_id: CollectionId,
) -> rusqlite::Result<Vec<SpecimenRow>> {
    conn.prepare_cached(
        "SELECT id, specimen_type_code, acceptance, received_at, revision FROM spc_specimens \
         WHERE collection_id = ?1 ORDER BY id",
    )?
    .query_map([collection_id], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    })?
    .collect()
}

/// The accession a specimen belongs to.
pub(crate) fn specimen_accession(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<Option<AccessionId>> {
    conn.prepare_cached(
        "SELECT col.accession_id FROM spc_specimens s \
         JOIN spc_v_collections col ON col.id = s.collection_id WHERE s.id = ?1",
    )?
    .query_row([specimen_id], |row| row.get(0))
    .optional()
}

/// Every specimen of a subject's accessions, with its collection time.
pub(crate) fn subject_specimens(
    conn: &Connection,
    subject_id: SubjectId,
) -> rusqlite::Result<Vec<SubjectSpecimen>> {
    conn.prepare_cached(
        "SELECT s.id, s.specimen_type_code, s.acceptance, s.received_at, a.id, \
         a.accession_number, a.order_id, col.precision, col.collected_at, \
         col.collected_offset_minutes, col.collected_date FROM spc_specimens s \
         JOIN spc_v_collections col ON col.id = s.collection_id \
         JOIN spc_accessions a ON a.id = col.accession_id \
         WHERE a.subject_id = ?1 ORDER BY s.received_at, s.id",
    )?
    .query_map([subject_id], |row| {
        Ok(SubjectSpecimen {
            specimen_id: row.get(0)?,
            specimen_type_code: row.get(1)?,
            acceptance: row.get(2)?,
            received_at: row.get(3)?,
            accession_id: row.get(4)?,
            accession_number: row.get(5)?,
            order_id: row.get(6)?,
            precision: row.get(7)?,
            collected_at: row.get(8)?,
            utc_offset_minutes: row.get(9)?,
            collected_date: row.get(10)?,
        })
    })?
    .collect()
}
