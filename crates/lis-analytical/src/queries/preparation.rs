//! Reopen preparation work after navigation or restart (UI-03).

use lis_catalog::MethodVersionId;
use lis_core::UtcTimestamp;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AnalyticalError, invalid};
use crate::model::{PrepBatchId, PrepBatchState};

/// A bounded, stable page of batches for one approved method version.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ListPrepBatchesInput {
    /// Exact method version; superseded versions retain their batches.
    pub method_version_id: MethodVersionId,
    /// Last batch ID of the previous page.
    pub after: Option<PrepBatchId>,
    /// Maximum rows, 1–200; callers normally request 50.
    pub limit: u32,
}

/// A preparation batch's directory row; steps are loaded only on opening it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepBatchSummary {
    /// Batch identity for reopening and linking.
    pub prep_batch_id: PrepBatchId,
    /// Exact method version.
    pub method_version_id: MethodVersionId,
    /// Open or completed.
    pub state: PrepBatchState,
    /// Actor who started it.
    pub created_by: String,
    /// Original start timestamp.
    pub created_at: UtcTimestamp,
    /// Completion timestamp, if completed.
    pub completed_at: Option<UtcTimestamp>,
    /// Optimistic revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// Recorded steps.
    #[ts(type = "number")]
    pub step_count: i64,
}

/// Lists all reachable pages in ID order, independent of wall-clock order.
/// The engine requires `perform_analysis`; no patient data is returned.
pub fn list_prep_batches(
    conn: &Connection,
    input: ListPrepBatchesInput,
) -> Result<Vec<PrepBatchSummary>, AnalyticalError> {
    if !(1..=200).contains(&input.limit) {
        return Err(invalid(
            "limit",
            "out_of_range",
            "Request 1 to 200 batches per page.",
        ));
    }
    lis_catalog::get_method_version(conn, input.method_version_id)?;
    Ok(crate::repo::lots::list_prep_batches(conn, &input)?)
}
