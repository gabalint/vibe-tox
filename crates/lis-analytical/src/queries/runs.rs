//! Run lists and audited run detail including patient/material context.

use lis_core::{InstrumentId, UtcTimestamp};
use rusqlite::Connection;

use super::{instrument_name, lot_view};
use crate::error::AnalyticalError;
use crate::model::{ExecutionId, ExecutionState, RunId, RunState};
use crate::repo::runs::{self as repo, PositionRow};
use crate::served::{CatalogCache, expected_analytes};
use crate::views::{ExecutionView, RunDetail, RunPositionView, RunSummary};

/// A run with its lots, positions and executions; lot expiry derived at `now`.
pub fn get_run(
    conn: &Connection,
    run_id: RunId,
    now: UtcTimestamp,
) -> Result<RunDetail, AnalyticalError> {
    let run = repo::run(conn, run_id)?.ok_or(AnalyticalError::NotFound("run"))?;
    let method = lis_catalog::get_method_version(conn, run.method_version_id)?;
    let lots = repo::run_lots(conn, run.id)?
        .into_iter()
        .map(|lot| lot_view(conn, lot, now))
        .collect::<Result<_, _>>()?;
    let mut cache = CatalogCache::default();
    let mut positions = Vec::new();
    for position in repo::run_positions(conn, run.id)? {
        positions.push(position_view(conn, &mut cache, position)?);
    }
    let allowed_actions = match run.state {
        RunState::Open => ["add_patient_position", "add_qc_position", "close_run"]
            .map(str::to_owned)
            .to_vec(),
        RunState::Closed => Vec::new(),
    };
    let resolution = repo::site_resolution(conn, run_id)?;
    Ok(RunDetail {
        laboratory_profile_id: run
            .laboratory_profile_id
            .or(resolution.as_ref().map(|r| r.0)),
        performing_site_source: resolution.as_ref().map(|r| r.1.clone()),
        performing_site_reason: resolution.map(|r| r.2),
        run_id: run.id,
        instrument_name: instrument_name(conn, run.instrument_id)?,
        instrument_id: run.instrument_id,
        method_version_id: run.method_version_id,
        method_code: method.method_code,
        method_version: method.version,
        state: run.state,
        created_by: run.created_by,
        created_by_display: None,
        created_at: run.created_at,
        closed_at: run.closed_at,
        close_reason: run.close_reason,
        revision: run.revision,
        lots,
        positions,
        allowed_actions,
    })
}

fn position_view(
    conn: &Connection,
    cache: &mut CatalogCache,
    position: PositionRow,
) -> Result<RunPositionView, AnalyticalError> {
    let execution = position
        .execution_id
        .map(|id| execution_view(conn, cache, id))
        .transpose()?;
    let patient_context = if execution.is_some() {
        let material = lis_specimens::find_container(conn, &position.sample_label)?;
        Some(crate::views::RunPatientContext {
            accession_id: material.accession_id,
            accession_number: material.accession_number,
            specimen_type_code: material.specimen_type_code,
            subject: material.subject,
        })
    } else {
        None
    };
    Ok(RunPositionView {
        patient_context,
        position_id: position.id,
        label: position.label,
        role: position.role,
        sample_label: position.sample_label,
        lot_id: position.lot_id,
        execution,
    })
}

fn execution_view(
    conn: &Connection,
    cache: &mut CatalogCache,
    id: ExecutionId,
) -> Result<ExecutionView, AnalyticalError> {
    let mut view = repo::execution(conn, id)?
        .ok_or(AnalyticalError::NotFound("execution"))?
        .view;
    view.component_ids = repo::execution_components(conn, id)?;
    view.expected_analytes = expected_analytes(conn, cache, &view.component_ids)?;
    if view.state == ExecutionState::Active {
        view.allowed_actions = vec!["fail_execution".to_owned()];
    }
    Ok(view)
}

/// When the run was created: with its instrument, how the run list
/// identifies a run.
pub fn run_created_at(conn: &Connection, run_id: RunId) -> Result<UtcTimestamp, AnalyticalError> {
    Ok(repo::run(conn, run_id)?
        .ok_or(AnalyticalError::NotFound("run"))?
        .created_at)
}

/// The newest 500 runs, optionally by state and instrument.
pub fn list_runs(
    conn: &Connection,
    state: Option<RunState>,
    instrument_id: Option<InstrumentId>,
) -> Result<Vec<RunSummary>, AnalyticalError> {
    Ok(repo::runs(conn, state, instrument_id)?)
}

/// A bounded run directory request; all filters persist across cursor pages.
#[derive(Debug, Clone, serde::Deserialize, ts_rs::TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ListRunsPageInput {
    /// Optional run state.
    pub state: Option<RunState>,
    /// Optional instrument.
    pub instrument_id: Option<InstrumentId>,
    /// Optional pinned method version.
    pub method_version_id: Option<lis_catalog::MethodVersionId>,
    /// Last run of the preceding page.
    pub after_run_id: Option<RunId>,
    /// Requested rows, 1–500.
    pub limit: u32,
}

/// A stable reachable page of runs, optionally scoped to a pinned method.
pub fn list_runs_page(
    conn: &Connection,
    input: ListRunsPageInput,
) -> Result<Vec<RunSummary>, AnalyticalError> {
    if !(1..=500).contains(&input.limit) {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Request 1 to 500 runs per page.",
        ));
    }
    if let Some(id) = input.after_run_id {
        repo::run(conn, id)?.ok_or(AnalyticalError::NotFound("run cursor"))?;
    }
    Ok(repo::runs_page(conn, &input)?)
}
