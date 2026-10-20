//! SQL for lots, preparation batches and preparation steps.

use lis_catalog::MethodVersionId;
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{CivilDate, LotId, LotState, MaterialKind, PrepBatchId, PrepStepId};
use crate::views::{PrepBatchView, PrepStepView};

pub(crate) struct NewLot<'a> {
    pub(crate) id: LotId,
    pub(crate) material_kind: MaterialKind,
    pub(crate) product_name: &'a str,
    pub(crate) lot_number: &'a str,
    pub(crate) level_text: Option<&'a str>,
    pub(crate) sample_label: Option<&'a str>,
    pub(crate) expires_on: CivilDate,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_lot(conn: &Connection, new: &NewLot<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_lots (id, material_kind, product_name, lot_number, level_text, \
         sample_label, expires_on, state, recorded_by, recorded_at, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?9, 1)",
    )?
    .execute(params![
        new.id,
        new.material_kind,
        new.product_name,
        new.lot_number,
        new.level_text,
        new.sample_label,
        new.expires_on.to_text(),
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn lot_exists(
    conn: &Connection,
    kind: MaterialKind,
    product_name: &str,
    lot_number: &str,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM ana_lots WHERE material_kind = ?1 AND product_name = ?2 \
         AND lot_number = ?3)",
    )?
    .query_row(params![kind, product_name, lot_number], |row| row.get(0))
}

/// A lot as stored; `expired` is derived by the caller.
pub(crate) struct LotRow {
    pub(crate) id: LotId,
    pub(crate) material_kind: MaterialKind,
    pub(crate) product_name: String,
    pub(crate) lot_number: String,
    pub(crate) level_text: Option<String>,
    pub(crate) sample_label: Option<String>,
    pub(crate) expires_on: String,
    pub(crate) state: LotState,
    pub(crate) recorded_at: UtcTimestamp,
    pub(crate) revision: i64,
}

const LOT_COLUMNS: &str = "id, material_kind, product_name, lot_number, level_text, \
     sample_label, expires_on, state, recorded_at, revision";

fn lot_from_row(row: &Row<'_>) -> rusqlite::Result<LotRow> {
    Ok(LotRow {
        id: row.get(0)?,
        material_kind: row.get(1)?,
        product_name: row.get(2)?,
        lot_number: row.get(3)?,
        level_text: row.get(4)?,
        sample_label: row.get(5)?,
        expires_on: row.get(6)?,
        state: row.get(7)?,
        recorded_at: row.get(8)?,
        revision: row.get(9)?,
    })
}

pub(crate) fn lot(conn: &Connection, id: LotId) -> rusqlite::Result<Option<LotRow>> {
    conn.prepare_cached(&format!("SELECT {LOT_COLUMNS} FROM ana_lots WHERE id = ?1"))?
        .query_row([id], lot_from_row)
        .optional()
}

/// Every lot, or the active ones, by product and lot number.
pub(crate) fn lots(conn: &Connection, active_only: bool) -> rusqlite::Result<Vec<LotRow>> {
    conn.prepare_cached(&format!(
        "SELECT {LOT_COLUMNS} FROM ana_lots WHERE ?1 = 0 OR state = 'active' \
         ORDER BY product_name, lot_number, material_kind"
    ))?
    .query_map([active_only], lot_from_row)?
    .collect()
}

pub(crate) fn withdraw_lot(
    conn: &Connection,
    id: LotId,
    revision: i64,
    reason: &str,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_lots SET state = 'withdrawn', withdrawn_by = ?2, withdrawn_at = ?3, \
         withdraw_reason = ?4, revision = ?5 WHERE id = ?1",
    )?
    .execute(params![id, by, at, reason, revision])?;
    Ok(())
}

pub(crate) fn insert_prep_batch(
    conn: &Connection,
    id: PrepBatchId,
    method_version_id: MethodVersionId,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_prep_batches (id, method_version_id, created_by, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![id, method_version_id, by, at])?;
    Ok(())
}

pub(crate) fn prep_batch(
    conn: &Connection,
    id: PrepBatchId,
) -> rusqlite::Result<Option<PrepBatchView>> {
    conn.prepare_cached(
        "SELECT id, method_version_id, state, created_by, created_at, completed_by, \
         completed_at, revision FROM ana_prep_batches WHERE id = ?1",
    )?
    .query_row([id], |row| {
        Ok(PrepBatchView {
            prep_batch_id: row.get(0)?,
            method_version_id: row.get(1)?,
            state: row.get(2)?,
            created_by: row.get(3)?,
            created_at: row.get(4)?,
            completed_by: row.get(5)?,
            completed_at: row.get(6)?,
            revision: row.get(7)?,
            steps: Vec::new(),
        })
    })
    .optional()
}

/// Raises an open batch's revision: a step was recorded.
pub(crate) fn touch_prep_batch(
    conn: &Connection,
    id: PrepBatchId,
    revision: i64,
) -> rusqlite::Result<()> {
    conn.prepare_cached("UPDATE ana_prep_batches SET revision = ?2 WHERE id = ?1")?
        .execute(params![id, revision])?;
    Ok(())
}

pub(crate) fn complete_prep_batch(
    conn: &Connection,
    id: PrepBatchId,
    revision: i64,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_prep_batches SET state = 'completed', completed_by = ?2, completed_at = ?3, \
         revision = ?4 WHERE id = ?1",
    )?
    .execute(params![id, by, at, revision])?;
    Ok(())
}

pub(crate) struct NewStep<'a> {
    pub(crate) batch_id: PrepBatchId,
    pub(crate) step_code: &'a str,
    pub(crate) lot_id: Option<LotId>,
    pub(crate) note: Option<&'a str>,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

/// Appends a step with the next sequence number of its batch.
pub(crate) fn insert_prep_step(
    conn: &Connection,
    new: &NewStep<'_>,
) -> rusqlite::Result<PrepStepId> {
    let id = PrepStepId::generate();
    conn.prepare_cached(
        "INSERT INTO ana_prep_steps (id, batch_id, seq, step_code, lot_id, note, performed_by, \
         performed_at) VALUES (?1, ?2, \
         (SELECT coalesce(max(seq), 0) + 1 FROM ana_prep_steps WHERE batch_id = ?2), \
         ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        id,
        new.batch_id,
        new.step_code,
        new.lot_id,
        new.note,
        new.by,
        new.at
    ])?;
    Ok(id)
}

pub(crate) fn prep_steps(
    conn: &Connection,
    batch_id: PrepBatchId,
) -> rusqlite::Result<Vec<PrepStepView>> {
    conn.prepare_cached(
        "SELECT id, seq, step_code, lot_id, note, performed_by, performed_at FROM ana_prep_steps \
         WHERE batch_id = ?1 ORDER BY seq",
    )?
    .query_map([batch_id], |row| {
        Ok(PrepStepView {
            step_id: row.get(0)?,
            seq: row.get(1)?,
            step_code: row.get(2)?,
            lot_id: row.get(3)?,
            note: row.get(4)?,
            performed_by: row.get(5)?,
            performed_at: row.get(6)?,
        })
    })?
    .collect()
}

/// The lots a batch's steps used.
pub(crate) fn prep_batch_lots(
    conn: &Connection,
    batch_id: PrepBatchId,
) -> rusqlite::Result<Vec<LotId>> {
    conn.prepare_cached(
        "SELECT DISTINCT lot_id FROM ana_prep_steps WHERE batch_id = ?1 AND lot_id IS NOT NULL",
    )?
    .query_map([batch_id], |row| row.get(0))?
    .collect()
}

pub(crate) fn list_prep_batches(
    conn: &Connection,
    input: &crate::ListPrepBatchesInput,
) -> rusqlite::Result<Vec<crate::PrepBatchSummary>> {
    conn.prepare_cached(
        "SELECT b.id, b.method_version_id, b.state, b.created_by, b.created_at, b.completed_at, \
         b.revision, (SELECT count(*) FROM ana_prep_steps s WHERE s.batch_id = b.id) \
         FROM ana_prep_batches b WHERE b.method_version_id = ?1 AND (?2 IS NULL OR b.id > ?2) \
         ORDER BY b.id LIMIT ?3",
    )?
    .query_map(
        params![input.method_version_id, input.after, input.limit],
        |row| {
            Ok(crate::PrepBatchSummary {
                prep_batch_id: row.get(0)?,
                method_version_id: row.get(1)?,
                state: row.get(2)?,
                created_by: row.get(3)?,
                created_at: row.get(4)?,
                completed_at: row.get(5)?,
                revision: row.get(6)?,
                step_count: row.get(7)?,
            })
        },
    )?
    .collect()
}
