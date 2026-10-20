//! Work-queue and billing reads (REQ-OPS-004, REQ-BIL-005). Identifiers,
//! codes and times only, oldest first.

use lis_orders::OrderComponentId;
use rusqlite::Connection;

use crate::dependency::qc_status;
use crate::error::AnalyticalError;
use crate::model::QcStatus;
use crate::repo::qc::{self as qc_repo, ScopeKey};
use crate::repo::{deviations, queues};
use crate::views::{ComponentExecution, DeviationView, QcWorkItem};

/// Every execution of the order component, active or failed, oldest first.
pub fn component_executions(
    conn: &Connection,
    component_id: OrderComponentId,
) -> Result<Vec<ComponentExecution>, AnalyticalError> {
    Ok(queues::component_executions(conn, component_id)?)
}

/// Run × analyte QC that needs a decision, at most `limit`, oldest first:
/// `pending` and `stale` wherever the current observations differ from the
/// cited ones or a QC conflict is open (the review gate's rule), and
/// `rejected` while the run is open, so its work can be failed or repeated
/// before the run closes. A closed run's rejection needs no further QC
/// action and leaves the queue.
pub fn qc_work_queue(conn: &Connection, limit: u32) -> Result<Vec<QcWorkItem>, AnalyticalError> {
    let mut items = Vec::new();
    for (run_id, analyte_id, since) in queues::changed_qc(conn)? {
        let current = qc_repo::current_assessment(conn, ScopeKey::RunAnalyte(run_id, analyte_id))?;
        let status = qc_status(current.map(|assessment| assessment.outcome), true);
        match status {
            QcStatus::Pending | QcStatus::Stale => items.push(QcWorkItem {
                run_id,
                analyte_id,
                status,
                since,
            }),
            // Listed below, while the run is open.
            QcStatus::Rejected | QcStatus::Accepted => {}
        }
    }
    for (run_id, analyte_id, since) in queues::rejected_on_open_runs(conn)? {
        items.push(QcWorkItem {
            run_id,
            analyte_id,
            status: QcStatus::Rejected,
            since,
        });
    }
    items.sort_by_key(|item| (item.since, item.run_id, item.analyte_id));
    items.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    Ok(items)
}

/// Open deviations, oldest first, at most `limit`. Descriptions and reasons
/// are staff-typed text about laboratory work, not patient data.
pub fn open_deviations(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<DeviationView>, AnalyticalError> {
    Ok(deviations::open(conn, limit)?)
}

/// Opens one deviation by its exact identity, including lot-scoped deviations
/// that can affect more than one run. The engine authorizes and audits this read.
pub fn get_deviation(
    conn: &Connection,
    id: crate::DeviationId,
) -> Result<DeviationView, AnalyticalError> {
    deviations::deviation(conn, id)?.ok_or(AnalyticalError::NotFound("deviation"))
}
