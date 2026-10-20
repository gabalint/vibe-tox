//! SQL for runs, run lots, executions, their components and run positions.

use lis_catalog::MethodVersionId;
use lis_core::{DecimalText, InstrumentId, UtcTimestamp};
use lis_orders::OrderComponentId;
use lis_specimens::ContainerId;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    AttemptKind, ExecutionId, ExecutionState, LotId, PositionId, PositionRole, PrepBatchId, RunId,
    RunState,
};
use crate::views::ExecutionView;

mod listings;
mod sites;
pub(crate) use listings::{runs, runs_page};
pub(crate) use sites::{insert_site_resolution, site_resolution};

/// Facts pinned when a new run is created; lifecycle defaults are owned by storage.
pub(crate) struct NewRun<'a> {
    pub(crate) id: RunId,
    pub(crate) instrument_id: InstrumentId,
    pub(crate) method_version_id: MethodVersionId,
    pub(crate) laboratory_profile_id: Option<lis_identity::LaboratoryProfileId>,
    pub(crate) created_by: &'a str,
    pub(crate) created_at: UtcTimestamp,
}

pub(crate) fn insert_run(conn: &Connection, run: &NewRun<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_runs (id, instrument_id, method_version_id, state, created_by, \
         created_at, revision, laboratory_profile_id) VALUES (?1, ?2, ?3, 'open', ?4, ?5, 1, ?6)",
    )?
    .execute(params![
        run.id,
        run.instrument_id,
        run.method_version_id,
        run.created_by,
        run.created_at,
        run.laboratory_profile_id
    ])?;
    Ok(())
}

pub(crate) fn insert_run_lot(
    conn: &Connection,
    run_id: RunId,
    lot_id: LotId,
) -> rusqlite::Result<()> {
    conn.prepare_cached("INSERT INTO ana_run_lots (run_id, lot_id) VALUES (?1, ?2)")?
        .execute(params![run_id, lot_id])?;
    Ok(())
}

/// A run as stored.
pub(crate) struct RunRow {
    pub(crate) id: RunId,
    pub(crate) instrument_id: InstrumentId,
    pub(crate) method_version_id: MethodVersionId,
    pub(crate) laboratory_profile_id: Option<lis_identity::LaboratoryProfileId>,
    pub(crate) state: RunState,
    pub(crate) created_by: String,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) closed_at: Option<UtcTimestamp>,
    pub(crate) close_reason: Option<String>,
    pub(crate) revision: i64,
}

pub(crate) fn run(conn: &Connection, id: RunId) -> rusqlite::Result<Option<RunRow>> {
    conn.prepare_cached(
        "SELECT id, instrument_id, method_version_id, state, created_by, created_at, closed_at, \
         close_reason, revision, laboratory_profile_id FROM ana_runs WHERE id = ?1",
    )?
    .query_row([id], |row| {
        Ok(RunRow {
            id: row.get(0)?,
            instrument_id: row.get(1)?,
            method_version_id: row.get(2)?,
            state: row.get(3)?,
            created_by: row.get(4)?,
            created_at: row.get(5)?,
            closed_at: row.get(6)?,
            close_reason: row.get(7)?,
            revision: row.get(8)?,
            laboratory_profile_id: row.get(9)?,
        })
    })
    .optional()
}

pub(crate) fn close_run(
    conn: &Connection,
    id: RunId,
    revision: i64,
    reason: Option<&str>,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_runs SET state = 'closed', closed_by = ?2, closed_at = ?3, close_reason = ?4, \
         revision = ?5 WHERE id = ?1",
    )?
    .execute(params![id, by, at, reason, revision])?;
    Ok(())
}

pub(crate) fn run_lots(conn: &Connection, run_id: RunId) -> rusqlite::Result<Vec<LotId>> {
    conn.prepare_cached("SELECT lot_id FROM ana_run_lots WHERE run_id = ?1 ORDER BY lot_id")?
        .query_map([run_id], |row| row.get(0))?
        .collect()
}

pub(crate) struct NewExecution<'a> {
    pub(crate) id: ExecutionId,
    pub(crate) container_id: ContainerId,
    pub(crate) method_version_id: MethodVersionId,
    pub(crate) attempt_kind: AttemptKind,
    pub(crate) related_execution_id: Option<ExecutionId>,
    pub(crate) dilution_factor: Option<&'a DecimalText>,
    pub(crate) prep_batch_id: Option<PrepBatchId>,
    pub(crate) reason: Option<&'a str>,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_execution(conn: &Connection, new: &NewExecution<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_executions (id, container_id, method_version_id, attempt_kind, \
         related_execution_id, dilution_factor, prep_batch_id, reason, state, created_by, \
         created_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', ?9, ?10, 1)",
    )?
    .execute(params![
        new.id,
        new.container_id,
        new.method_version_id,
        new.attempt_kind,
        new.related_execution_id,
        new.dilution_factor,
        new.prep_batch_id,
        new.reason,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn insert_execution_component(
    conn: &Connection,
    execution_id: ExecutionId,
    component_id: OrderComponentId,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_execution_components (execution_id, order_component_id) VALUES (?1, ?2)",
    )?
    .execute(params![execution_id, component_id])?;
    Ok(())
}

pub(crate) fn execution_components(
    conn: &Connection,
    execution_id: ExecutionId,
) -> rusqlite::Result<Vec<OrderComponentId>> {
    conn.prepare_cached(
        "SELECT order_component_id FROM ana_execution_components WHERE execution_id = ?1 \
         ORDER BY order_component_id",
    )?
    .query_map([execution_id], |row| row.get(0))?
    .collect()
}

/// Whether any active execution serves `component_id`.
pub(crate) fn component_in_progress(
    conn: &Connection,
    component_id: OrderComponentId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM ana_execution_components c \
         JOIN ana_executions e ON e.id = c.execution_id \
         WHERE c.order_component_id = ?1 AND e.state = 'active')",
    )?
    .query_row([component_id], |row| row.get(0))
}

const EXECUTION_COLUMNS: &str = "id, container_id, attempt_kind, related_execution_id, \
     dilution_factor, prep_batch_id, reason, state, state_reason, revision, method_version_id";

/// An execution as stored, with its method version.
pub(crate) struct ExecutionRow {
    pub(crate) view: ExecutionView,
    pub(crate) method_version_id: MethodVersionId,
}

fn execution_from_row(row: &Row<'_>) -> rusqlite::Result<ExecutionRow> {
    Ok(ExecutionRow {
        view: ExecutionView {
            execution_id: row.get(0)?,
            container_id: row.get(1)?,
            attempt_kind: row.get(2)?,
            related_execution_id: row.get(3)?,
            dilution_factor: row.get(4)?,
            prep_batch_id: row.get(5)?,
            reason: row.get(6)?,
            state: row.get(7)?,
            state_reason: row.get(8)?,
            revision: row.get(9)?,
            component_ids: Vec::new(),
            expected_analytes: 0,
            allowed_actions: Vec::new(),
        },
        method_version_id: row.get(10)?,
    })
}

pub(crate) fn execution(
    conn: &Connection,
    id: ExecutionId,
) -> rusqlite::Result<Option<ExecutionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {EXECUTION_COLUMNS} FROM ana_executions WHERE id = ?1"
    ))?
    .query_row([id], execution_from_row)
    .optional()
}

pub(crate) fn fail_execution(
    conn: &Connection,
    id: ExecutionId,
    revision: i64,
    reason: &str,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_executions SET state = ?2, state_reason = ?3, revision = ?4 WHERE id = ?1",
    )?
    .execute(params![id, ExecutionState::Failed, reason, revision])?;
    Ok(())
}

pub(crate) struct NewPosition<'a> {
    pub(crate) id: PositionId,
    pub(crate) run_id: RunId,
    pub(crate) label: &'a str,
    pub(crate) role: PositionRole,
    pub(crate) execution_id: Option<ExecutionId>,
    pub(crate) lot_id: Option<LotId>,
    pub(crate) sample_label: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_position(conn: &Connection, new: &NewPosition<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_run_positions (id, run_id, position_label, role, execution_id, lot_id, \
         sample_label, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        new.id,
        new.run_id,
        new.label,
        new.role,
        new.execution_id,
        new.lot_id,
        new.sample_label,
        new.by,
        new.at
    ])?;
    Ok(())
}

/// Whether the run already has a position with this label or sample label.
pub(crate) fn position_taken(
    conn: &Connection,
    run_id: RunId,
    label: &str,
    sample_label: &str,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM ana_run_positions WHERE run_id = ?1 \
         AND (position_label = ?2 OR sample_label = ?3))",
    )?
    .query_row(params![run_id, label, sample_label], |row| row.get(0))
}

/// A position as published in `ana_v_positions`, with its label and creation time.
#[derive(Clone)]
pub(crate) struct PositionRow {
    pub(crate) id: PositionId,
    pub(crate) run_id: RunId,
    pub(crate) run_state: RunState,
    pub(crate) instrument_id: InstrumentId,
    pub(crate) method_version_id: MethodVersionId,
    pub(crate) role: PositionRole,
    pub(crate) execution_id: Option<ExecutionId>,
    pub(crate) execution_state: Option<ExecutionState>,
    pub(crate) prep_batch_id: Option<PrepBatchId>,
    pub(crate) lot_id: Option<LotId>,
    pub(crate) label: String,
    pub(crate) sample_label: String,
    pub(crate) created_at: UtcTimestamp,
}

const POSITION_COLUMNS: &str = "v.position_id, v.run_id, v.run_state, v.instrument_id, \
     v.method_version_id, v.role, v.execution_id, v.prep_batch_id, v.lot_id, p.position_label, \
     v.sample_label, p.created_at, v.execution_state";

fn position_from_row(row: &Row<'_>) -> rusqlite::Result<PositionRow> {
    Ok(PositionRow {
        id: row.get(0)?,
        run_id: row.get(1)?,
        run_state: row.get(2)?,
        instrument_id: row.get(3)?,
        method_version_id: row.get(4)?,
        role: row.get(5)?,
        execution_id: row.get(6)?,
        prep_batch_id: row.get(7)?,
        lot_id: row.get(8)?,
        label: row.get(9)?,
        sample_label: row.get(10)?,
        created_at: row.get(11)?,
        execution_state: row.get(12)?,
    })
}

pub(crate) fn position(conn: &Connection, id: PositionId) -> rusqlite::Result<Option<PositionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {POSITION_COLUMNS} FROM ana_v_positions v \
         JOIN ana_run_positions p ON p.id = v.position_id WHERE v.position_id = ?1"
    ))?
    .query_row([id], position_from_row)
    .optional()
}

/// The positions of a run, in the order added.
pub(crate) fn run_positions(
    conn: &Connection,
    run_id: RunId,
) -> rusqlite::Result<Vec<PositionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {POSITION_COLUMNS} FROM ana_v_positions v \
         JOIN ana_run_positions p ON p.id = v.position_id WHERE v.run_id = ?1 \
         ORDER BY p.created_at, p.id"
    ))?
    .query_map([run_id], position_from_row)?
    .collect()
}

/// Positions with `sample_label` on open runs of `instrument_id` (brief §3.3).
pub(crate) fn open_positions_by_sample_label(
    conn: &Connection,
    instrument_id: InstrumentId,
    sample_label: &str,
) -> rusqlite::Result<Vec<PositionRow>> {
    conn.prepare_cached(&format!(
        "SELECT {POSITION_COLUMNS} FROM ana_v_positions v \
         JOIN ana_run_positions p ON p.id = v.position_id \
         WHERE v.instrument_id = ?1 AND v.sample_label = ?2 AND v.run_state = 'open' LIMIT 2"
    ))?
    .query_map(params![instrument_id, sample_label], position_from_row)?
    .collect()
}

/// The run holding an execution's position.
pub(crate) fn execution_run(
    conn: &Connection,
    execution_id: ExecutionId,
) -> rusqlite::Result<Option<RunId>> {
    conn.prepare_cached("SELECT run_id FROM ana_run_positions WHERE execution_id = ?1")?
        .query_row([execution_id], |row| row.get(0))
        .optional()
}
