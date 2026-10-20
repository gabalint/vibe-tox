//! SQL for changed QC rows awaiting a decision (brief §3.5, R5).

use lis_catalog::AnalyteId;
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{QcConflictId, QcConflictState, QcObservationId, RunId};
use crate::repo::qc::NewObservation;
use crate::views::QcConflictView;

/// Records a conflicting row against the observation it contradicts, unless
/// an identical open conflict already holds it. Returns the new ID, if any.
pub(crate) fn insert(
    conn: &Connection,
    new: &NewObservation<'_>,
    conflicts_with: QcObservationId,
) -> rusqlite::Result<Option<QcConflictId>> {
    let id = QcConflictId::generate();
    let inserted = conn
        .prepare_cached(
            "INSERT INTO ana_qc_conflicts (id, position_id, conflicts_with_id, receipt_id, \
             record_path, source_test_code, analyte_id, value_text, units_text, flags_text, \
             status_text, state, recorded_at, revision) \
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'open', ?12, 1 \
             WHERE NOT EXISTS (SELECT 1 FROM ana_qc_conflicts WHERE state = 'open' \
             AND conflicts_with_id = ?3 AND source_test_code IS ?6 AND analyte_id IS ?7 \
             AND value_text IS ?8 AND units_text IS ?9 AND flags_text IS ?10 \
             AND status_text IS ?11)",
        )?
        .execute(params![
            id,
            new.position_id,
            conflicts_with,
            new.receipt_id,
            new.record_path,
            new.source_test_code,
            new.analyte_id,
            new.row.value_text,
            new.row.units_text,
            new.row.flags_text,
            new.row.status_text,
            new.at
        ])?;
    Ok((inserted == 1).then_some(id))
}

const COLUMNS: &str = "c.id, c.position_id, c.conflicts_with_id, c.source_test_code, \
     c.analyte_id, c.value_text, c.units_text, c.flags_text, c.status_text, c.state, \
     c.observation_id, c.reason, c.resolved_by, c.resolved_at, c.recorded_at, c.revision, \
     c.receipt_id, c.record_path";

fn from_row(row: &Row<'_>) -> rusqlite::Result<ConflictRow> {
    Ok(ConflictRow {
        view: QcConflictView {
            conflict_id: row.get(0)?,
            position_id: row.get(1)?,
            conflicts_with_id: row.get(2)?,
            source_test_code: row.get(3)?,
            analyte_id: row.get(4)?,
            value_text: row.get(5)?,
            units_text: row.get(6)?,
            flags_text: row.get(7)?,
            status_text: row.get(8)?,
            state: row.get(9)?,
            observation_id: row.get(10)?,
            reason: row.get(11)?,
            resolved_by: row.get(12)?,
            resolved_at: row.get(13)?,
            recorded_at: row.get(14)?,
            revision: row.get(15)?,
        },
        receipt_id: row.get(16)?,
        record_path: row.get(17)?,
    })
}

/// A conflict as stored, with its provenance.
pub(crate) struct ConflictRow {
    pub(crate) view: QcConflictView,
    pub(crate) receipt_id: lis_interfaces::ReceiptId,
    pub(crate) record_path: String,
}

pub(crate) fn conflict(
    conn: &Connection,
    id: QcConflictId,
) -> rusqlite::Result<Option<ConflictRow>> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ana_qc_conflicts c WHERE c.id = ?1"
    ))?
    .query_row([id], from_row)
    .optional()
}

/// Every conflict of a run, open ones first, then by arrival.
pub(crate) fn run_conflicts(
    conn: &Connection,
    run_id: RunId,
) -> rusqlite::Result<Vec<QcConflictView>> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ana_qc_conflicts c \
         JOIN ana_run_positions p ON p.id = c.position_id WHERE p.run_id = ?1 \
         ORDER BY c.state <> 'open', c.recorded_at, c.id"
    ))?
    .query_map([run_id], |row| from_row(row).map(|found| found.view))?
    .collect()
}

/// Whether an open conflict on the run affects `analyte_id`: the row maps
/// to it, or the observation it contradicts does.
pub(crate) fn open_on_run_analyte(
    conn: &Connection,
    run_id: RunId,
    analyte_id: AnalyteId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM ana_qc_conflicts c \
         JOIN ana_run_positions p ON p.id = c.position_id \
         JOIN ana_qc_observations o ON o.id = c.conflicts_with_id \
         WHERE p.run_id = ?1 AND c.state = 'open' \
         AND (c.analyte_id = ?2 OR o.analyte_id = ?2))",
    )?
    .query_row(params![run_id, analyte_id], |row| row.get(0))
}

/// A person's decision on an open conflict.
pub(crate) struct Resolution<'a> {
    pub(crate) state: QcConflictState,
    pub(crate) observation_id: Option<QcObservationId>,
    pub(crate) reason: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn resolve(
    conn: &Connection,
    id: QcConflictId,
    revision: i64,
    resolution: &Resolution<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_qc_conflicts SET state = ?2, observation_id = ?3, reason = ?4, \
         resolved_by = ?5, resolved_at = ?6, revision = ?7 WHERE id = ?1",
    )?
    .execute(params![
        id,
        resolution.state,
        resolution.observation_id,
        resolution.reason,
        resolution.by,
        resolution.at,
        revision
    ])?;
    Ok(())
}
