//! QC of a run: observations, assessments and the deviations that affect it.

use rusqlite::Connection;

use crate::deviation_targets::{deviations_on_targets, position_level_targets, run_level_targets};
use crate::error::AnalyticalError;
use crate::model::{QcAssessmentId, RunId};
use crate::repo::qc::{self as repo, ScopeKey};
use crate::repo::{conflicts, runs};
use crate::views::{
    PrepBatchQcView, QcAssessmentHistory, QcAssessmentView, QcObservationView, RunQcAnalyteView,
    RunQcView,
};

/// Every QC observation of a run per analyte of its method version, its QC
/// conflicts, the current and past assessments of each scope, and every
/// deviation on the run, its instrument, lots, positions and preparation
/// batches.
pub fn get_run_qc(conn: &Connection, run_id: RunId) -> Result<RunQcView, AnalyticalError> {
    let run = runs::run(conn, run_id)?.ok_or(AnalyticalError::NotFound("run"))?;
    let method = lis_catalog::get_method_version(conn, run.method_version_id)?;
    let mut observations = repo::run_observations(conn, run.id)?;
    let mut analytes = Vec::new();
    for analyte in &method.analytes {
        let analyte_id = analyte.limits.analyte_id;
        let (own, rest): (Vec<QcObservationView>, Vec<QcObservationView>) = observations
            .into_iter()
            .partition(|observation| observation.analyte_id == Some(analyte_id));
        observations = rest;
        analytes.push(RunQcAnalyteView {
            analyte_id,
            code: analyte.analyte_code.clone(),
            observations: own,
            assessments: history(conn, ScopeKey::RunAnalyte(run.id, analyte_id))?,
        });
    }
    let positions = runs::run_positions(conn, run.id)?;
    let mut targets = run_level_targets(conn, run.instrument_id, run.id)?;
    let mut prep_batches: Vec<PrepBatchQcView> = Vec::new();
    for position in &positions {
        targets.extend(position_level_targets(conn, position)?);
        let Some(batch) = position.prep_batch_id else {
            continue;
        };
        if !prep_batches.iter().any(|seen| seen.prep_batch_id == batch) {
            prep_batches.push(PrepBatchQcView {
                prep_batch_id: batch,
                assessments: history(conn, ScopeKey::PrepBatch(batch))?,
            });
        }
    }
    Ok(RunQcView {
        run_id: run.id,
        analytes,
        unmapped_observations: observations,
        conflicts: conflicts::run_conflicts(conn, run.id)?,
        run_assessments: history(conn, ScopeKey::Run(run.id))?,
        prep_batches,
        deviations: deviations_on_targets(conn, &targets)?,
    })
}

/// The current assessment of a scope and all its assessments, newest first.
pub(crate) fn history(
    conn: &Connection,
    key: ScopeKey,
) -> Result<QcAssessmentHistory, AnalyticalError> {
    let current = repo::current_assessment(conn, key)?.map(|c| c.assessment_id);
    let mut history = repo::assessments(conn, key)?;
    for assessment in &mut history {
        assessment.evidence_observation_ids = repo::evidence(conn, assessment.assessment_id)?;
    }
    Ok(QcAssessmentHistory {
        current: history
            .iter()
            .find(|assessment| Some(assessment.assessment_id) == current)
            .cloned(),
        history,
    })
}

/// One assessment of a scope, with its evidence.
pub(crate) fn qc_assessment(
    conn: &Connection,
    key: ScopeKey,
    id: QcAssessmentId,
) -> Result<QcAssessmentView, AnalyticalError> {
    history(conn, key)?
        .history
        .into_iter()
        .find(|assessment| assessment.assessment_id == id)
        .ok_or(AnalyticalError::NotFound("QC assessment"))
}
