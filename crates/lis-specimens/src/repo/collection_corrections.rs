//! SQL for immutable collection corrections and their quality review.

use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use crate::collection_history::{
    CollectionCorrectionId, CollectionCorrectionView, CollectionReviewQueueItem,
};
use crate::input::CollectionTime;
use crate::{AccessionId, CollectionId, CollectionView, SpecimenError, SpecimenId};

pub(crate) fn original(
    conn: &Connection,
    accession: AccessionId,
) -> Result<CollectionView, SpecimenError> {
    let mut current = super::accessions::collection(conn, accession)?;
    let (precision, at, offset, date) = conn.query_row(
        "SELECT precision, collected_at, collected_offset_minutes, collected_date FROM spc_collections WHERE accession_id = ?1",
        [accession], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;
    current.precision = precision;
    current.collected_at = at;
    current.utc_offset_minutes = offset;
    current.collected_date = date;
    Ok(current)
}

pub(crate) fn history(
    conn: &Connection,
    collection: CollectionId,
) -> Result<Vec<CollectionCorrectionView>, SpecimenError> {
    Ok(conn.prepare_cached(
        "SELECT c.id, c.revision, c.precision, c.collected_at, c.collected_offset_minutes, c.collected_date, \
         c.source_document_ref, c.reason, c.recorded_by, c.recorded_at, r.reviewed_at, r.reason, r.reviewed_by \
         FROM spc_collection_corrections c LEFT JOIN spc_collection_correction_reviews r ON r.correction_id = c.id \
         WHERE c.collection_id = ?1 ORDER BY c.revision")?
        .query_map([collection], |row| {
            let resolved_at = row.get(10)?;
            Ok(CollectionCorrectionView {
                correction_id: row.get(0)?, revision: row.get(1)?, precision: row.get(2)?,
                collected_at: row.get(3)?, utc_offset_minutes: row.get(4)?, collected_date: row.get(5)?,
                source_document_ref: row.get(6)?, reason: row.get(7)?, recorded_by: row.get(8)?, recorded_at: row.get(9)?,
                resolved_at, resolution_reason: row.get(11)?, resolved_by: row.get(12)?,
                allowed_actions: if resolved_at.is_none() { vec!["resolve_collection_correction".to_owned()] } else { Vec::new() },
            })
        })?.collect::<Result<_, _>>()?)
}

pub(crate) fn insert(
    ctx: &CommandContext<'_>,
    id: CollectionCorrectionId,
    collection: CollectionId,
    revision: i64,
    time: &CollectionTime,
    evidence: (&str, &str),
) -> Result<(), SpecimenError> {
    ctx.tx().execute("INSERT INTO spc_collection_corrections \
        (id, collection_id, revision, precision, collected_at, collected_offset_minutes, collected_date, source_document_ref, reason, recorded_by, recorded_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![id, collection, revision, time.precision, time.collected_at, time.offset_minutes, time.date,
            evidence.0, evidence.1, ctx.principal().actor().id_text(), ctx.now()])?;
    Ok(())
}

pub(crate) fn accession_for_correction(
    conn: &Connection,
    correction: CollectionCorrectionId,
) -> Result<AccessionId, SpecimenError> {
    conn.query_row("SELECT c.accession_id FROM spc_collections c JOIN spc_collection_corrections r ON r.collection_id = c.id WHERE r.id = ?1",
        [correction], |row| row.get(0)).optional()?.ok_or(SpecimenError::NotFound("collection correction"))
}

pub(crate) fn resolve(
    ctx: &CommandContext<'_>,
    correction: CollectionCorrectionId,
    reason: &str,
) -> Result<(), SpecimenError> {
    let exists: bool = ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM spc_collection_correction_reviews WHERE correction_id = ?1)",
        [correction],
        |row| row.get(0),
    )?;
    if exists {
        return Err(SpecimenError::AlreadyClosed("collection correction"));
    }
    ctx.tx().execute("INSERT INTO spc_collection_correction_reviews (correction_id, reason, reviewed_by, reviewed_at) VALUES (?1, ?2, ?3, ?4)",
        params![correction, reason, ctx.principal().actor().id_text(), ctx.now()])?;
    Ok(())
}

pub(crate) fn review_pending(
    conn: &Connection,
    specimen: SpecimenId,
) -> Result<bool, SpecimenError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM spc_collection_corrections c \
        JOIN spc_specimens s ON s.collection_id = c.collection_id \
        LEFT JOIN spc_collection_correction_reviews r ON r.correction_id = c.id \
        WHERE s.id = ?1 AND r.correction_id IS NULL)",
        [specimen],
        |row| row.get(0),
    )?)
}

pub(crate) fn pending(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<CollectionReviewQueueItem>, SpecimenError> {
    Ok(conn.prepare_cached("SELECT a.id, a.accession_number, min(c.recorded_at) AS oldest \
        FROM spc_collection_corrections c JOIN spc_collections col ON col.id = c.collection_id \
        JOIN spc_accessions a ON a.id = col.accession_id \
        LEFT JOIN spc_collection_correction_reviews r ON r.correction_id = c.id \
        WHERE r.correction_id IS NULL GROUP BY a.id, a.accession_number ORDER BY oldest, a.id LIMIT ?1")?
        .query_map([limit], |row| Ok(CollectionReviewQueueItem {
            accession_id: row.get(0)?, accession_number: row.get(1)?, recorded_at: row.get(2)?,
        }))?.collect::<Result<_, _>>()?)
}
