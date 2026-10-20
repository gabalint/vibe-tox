//! Durable expected-analyte reconciliation (REQ-OPS-013, WFREV-01).

use lis_analytical::{ExecutionId, PositionId, RunId};
use lis_core::{InstrumentId, UtcTimestamp};
use lis_specimens::SpecimenId;
use rusqlite::{Connection, params};

use super::queues::bounded;
use crate::ResultError;

/// An active patient execution with at least one unresolved expected analyte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwaitingResultRow {
    pub run_id: RunId,
    pub instrument_id: InstrumentId,
    pub position_id: PositionId,
    pub execution_id: ExecutionId,
    pub specimen_id: SpecimenId,
    pub accession_number: String,
    pub since: UtcTimestamp,
}

/// Reconciles every active execution against its frozen ordered analytes.
/// Partial files and run closure do not finish the task. Failed executions,
/// cancelled components and explicit terminal result dispositions do.
/// No current-run page cap is applied before determining pending work.
pub fn awaiting_results(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<AwaitingResultRow>, ResultError> {
    let limit = bounded(limit) as usize;
    let mut statement = conn.prepare_cached(
        "SELECT p.run_id, p.instrument_id, p.position_id, p.execution_id, \
         c.specimen_id, c.accession_number, p.run_created_at \
         FROM ana_v_positions p JOIN spc_v_containers c ON c.container_id = p.container_id \
         WHERE p.role = 'patient' AND p.execution_state = 'active' \
         ORDER BY p.run_created_at, p.position_id",
    )?;
    let candidates = statement.query_map([], |row| {
        Ok(AwaitingResultRow {
            run_id: row.get(0)?,
            instrument_id: row.get(1)?,
            position_id: row.get(2)?,
            execution_id: row.get(3)?,
            specimen_id: row.get(4)?,
            accession_number: row.get(5)?,
            since: row.get(6)?,
        })
    })?;
    let mut pending = Vec::new();
    for candidate in candidates {
        if pending.len() == limit {
            break;
        }
        let row = candidate?;
        if missing_analyte(conn, &row)? {
            pending.push(row);
        }
    }
    Ok(pending)
}

fn missing_analyte(conn: &Connection, row: &AwaitingResultRow) -> Result<bool, ResultError> {
    for component_id in lis_analytical::execution_components(conn, row.execution_id)? {
        let (version_id, position, active): (lis_catalog::OrderableVersionId, i64, bool) = conn.query_row(
            "SELECT catalog_orderable_version_id, catalog_component_position, component_state = 'active' \
             FROM ord_v_components WHERE component_id = ?1", [component_id],
            |record| Ok((record.get(0)?, record.get(1)?, record.get(2)?)),
        )?;
        if !active {
            continue;
        }
        let version = lis_catalog::get_orderable_version(conn, version_id)?;
        let component = version
            .components
            .iter()
            .find(|component| component.position == position)
            .ok_or(ResultError::NotFound("frozen order component"))?;
        for analyte in &component.analytes {
            let reconciled: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM res_measurements WHERE position_id = ?1 AND analyte_id = ?2) \
                 OR EXISTS(SELECT 1 FROM res_results r JOIN res_result_revisions rr ON rr.id = r.current_revision_id \
                 WHERE r.order_component_id = ?3 AND r.analyte_id = ?2 AND rr.specimen_id = ?4 \
                 AND rr.cause = 'disposition' AND rr.outcome IN ('not_performed','invalid','indeterminate'))",
                params![row.position_id, analyte.analyte_id, component_id, row.specimen_id], |record| record.get(0),
            )?;
            if !reconciled {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
