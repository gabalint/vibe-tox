//! SQL for the work-queue and billing reads (REQ-OPS-004, REQ-BIL-005).

use lis_catalog::AnalyteId;
use lis_core::UtcTimestamp;
use lis_orders::OrderComponentId;
use rusqlite::Connection;

use crate::model::RunId;
use crate::views::ComponentExecution;

/// Executions linked to an order component, oldest first.
pub(crate) fn component_executions(
    conn: &Connection,
    component_id: OrderComponentId,
) -> rusqlite::Result<Vec<ComponentExecution>> {
    conn.prepare_cached(
        "SELECT e.id, e.attempt_kind, e.state FROM ana_execution_components c \
         JOIN ana_executions e ON e.id = c.execution_id WHERE c.order_component_id = ?1 \
         ORDER BY e.created_at, e.id",
    )?
    .query_map([component_id], |row| {
        Ok(ComponentExecution {
            execution_id: row.get(0)?,
            attempt_kind: row.get(1)?,
            state: row.get(2)?,
        })
    })?
    .collect()
}

/// Run × analyte pairs whose QC data differs from what their current
/// run × analyte assessment cites, or that have no assessment yet, with the
/// earliest time of that difference: a current observation it does not
/// cite, an observation superseding one it cites, or an open QC conflict on
/// either analyte the conflict touches (the review gate's rule).
pub(crate) fn changed_qc(
    conn: &Connection,
) -> rusqlite::Result<Vec<(RunId, AnalyteId, UtcTimestamp)>> {
    conn.prepare_cached(
        "SELECT run_id, analyte_id, min(since) FROM ( \
           SELECT p.run_id, o.analyte_id, o.recorded_at AS since FROM ana_qc_observations o \
           JOIN ana_run_positions p ON p.id = o.position_id \
           WHERE o.analyte_id IS NOT NULL \
           AND NOT EXISTS (SELECT 1 FROM ana_qc_observations s WHERE s.supersedes_id = o.id) \
           AND NOT EXISTS (SELECT 1 FROM ana_qc_assessment_evidence e \
               JOIN ana_qc_assessments a ON a.id = e.assessment_id \
               WHERE e.observation_id = o.id AND a.scope = 'run_analyte' \
               AND NOT EXISTS (SELECT 1 FROM ana_qc_assessments s WHERE s.supersedes_id = a.id)) \
         UNION ALL \
           SELECT a.run_id, a.analyte_id, s.recorded_at FROM ana_qc_assessments a \
           JOIN ana_qc_assessment_evidence e ON e.assessment_id = a.id \
           JOIN ana_qc_observations s ON s.supersedes_id = e.observation_id \
           WHERE a.scope = 'run_analyte' \
           AND NOT EXISTS (SELECT 1 FROM ana_qc_assessments n WHERE n.supersedes_id = a.id) \
         UNION ALL \
           SELECT p.run_id, c.analyte_id, c.recorded_at FROM ana_qc_conflicts c \
           JOIN ana_run_positions p ON p.id = c.position_id \
           WHERE c.state = 'open' AND c.analyte_id IS NOT NULL \
         UNION ALL \
           SELECT p.run_id, o.analyte_id, c.recorded_at FROM ana_qc_conflicts c \
           JOIN ana_run_positions p ON p.id = c.position_id \
           JOIN ana_qc_observations o ON o.id = c.conflicts_with_id \
           WHERE c.state = 'open' AND o.analyte_id IS NOT NULL \
         ) GROUP BY run_id, analyte_id",
    )?
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
    .collect()
}

/// Run × analyte pairs of open runs whose current assessment rejects them,
/// with the assessment time.
pub(crate) fn rejected_on_open_runs(
    conn: &Connection,
) -> rusqlite::Result<Vec<(RunId, AnalyteId, UtcTimestamp)>> {
    conn.prepare_cached(
        "SELECT a.run_id, a.analyte_id, a.assessed_at FROM ana_qc_assessments a \
         JOIN ana_runs r ON r.id = a.run_id \
         WHERE a.scope = 'run_analyte' AND a.outcome = 'rejected' AND r.state = 'open' \
         AND NOT EXISTS (SELECT 1 FROM ana_qc_assessments s WHERE s.supersedes_id = a.id)",
    )?
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
    .collect()
}
