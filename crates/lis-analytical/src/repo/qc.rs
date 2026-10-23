//! SQL for QC observations, QC assessments and their evidence.

use lis_catalog::AnalyteId;
use lis_core::UtcTimestamp;
use lis_interfaces::ReceiptId;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    PositionId, PrepBatchId, QcAssessmentId, QcObservationId, QcOutcome, QcScope, RunId,
};
use crate::reexport::SourceRow;
use crate::views::{QcAssessmentView, QcObservationView};

pub(crate) struct NewObservation<'a> {
    pub(crate) position_id: PositionId,
    pub(crate) receipt_id: ReceiptId,
    pub(crate) record_path: &'a str,
    pub(crate) source_test_code: &'a str,
    pub(crate) analyte_id: Option<AnalyteId>,
    pub(crate) row: &'a SourceRow,
    pub(crate) supersedes: Option<QcObservationId>,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_observation(
    conn: &Connection,
    new: &NewObservation<'_>,
) -> rusqlite::Result<QcObservationId> {
    let id = QcObservationId::generate();
    conn.prepare_cached(
        "INSERT INTO ana_qc_observations (id, position_id, receipt_id, record_path, \
         source_test_code, analyte_id, value_text, units_text, flags_text, status_text, \
         supersedes_id, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?
    .execute(params![
        id,
        new.position_id,
        new.receipt_id,
        new.record_path,
        new.source_test_code,
        new.analyte_id,
        new.row.value_text,
        new.row.units_text,
        new.row.flags_text,
        new.row.status_text,
        new.supersedes,
        new.at
    ])?;
    Ok(id)
}

/// The current (not superseded) observation for a position and test code.
pub(crate) fn current_observation(
    conn: &Connection,
    position_id: PositionId,
    source_test_code: &str,
) -> rusqlite::Result<Option<(QcObservationId, SourceRow)>> {
    conn.prepare_cached(
        "SELECT o.id, o.value_text, o.units_text, o.flags_text, o.status_text \
         FROM ana_qc_observations o WHERE o.position_id = ?1 AND o.source_test_code = ?2 \
         AND NOT EXISTS (SELECT 1 FROM ana_qc_observations s WHERE s.supersedes_id = o.id)",
    )?
    .query_row(params![position_id, source_test_code], |row| {
        Ok((
            row.get(0)?,
            SourceRow {
                value_text: row.get(1)?,
                units_text: row.get(2)?,
                flags_text: row.get(3)?,
                status_text: row.get(4)?,
            },
        ))
    })
    .optional()
}

/// The current observations of one analyte on a run.
pub(crate) fn current_run_analyte_observations(
    conn: &Connection,
    run_id: RunId,
    analyte_id: AnalyteId,
) -> rusqlite::Result<Vec<QcObservationId>> {
    conn.prepare_cached(
        "SELECT o.id FROM ana_qc_observations o JOIN ana_run_positions p ON p.id = o.position_id \
         WHERE p.run_id = ?1 AND o.analyte_id = ?2 \
         AND NOT EXISTS (SELECT 1 FROM ana_qc_observations s WHERE s.supersedes_id = o.id) \
         ORDER BY o.id",
    )?
    .query_map(params![run_id, analyte_id], |row| row.get(0))?
    .collect()
}

/// The run an observation's position belongs to, if the observation exists.
pub(crate) fn observation_run(
    conn: &Connection,
    observation_id: QcObservationId,
) -> rusqlite::Result<Option<RunId>> {
    conn.prepare_cached(
        "SELECT p.run_id FROM ana_qc_observations o \
         JOIN ana_run_positions p ON p.id = o.position_id WHERE o.id = ?1",
    )?
    .query_row([observation_id], |row| row.get(0))
    .optional()
}

/// Every observation of a run, superseded ones included.
pub(crate) fn run_observations(
    conn: &Connection,
    run_id: RunId,
) -> rusqlite::Result<Vec<QcObservationView>> {
    conn.prepare_cached(
        "SELECT o.id, o.position_id, p.position_label, p.lot_id, o.source_test_code, \
         o.analyte_id, o.value_text, o.units_text, o.flags_text, o.status_text, \
         EXISTS (SELECT 1 FROM ana_qc_observations s WHERE s.supersedes_id = o.id), o.recorded_at \
         FROM ana_qc_observations o JOIN ana_run_positions p ON p.id = o.position_id \
         WHERE p.run_id = ?1 ORDER BY p.created_at, p.id, o.source_test_code, o.recorded_at, o.id",
    )?
    .query_map([run_id], |row| {
        Ok(QcObservationView {
            observation_id: row.get(0)?,
            position_id: row.get(1)?,
            position_label: row.get(2)?,
            lot_id: row.get(3)?,
            source_test_code: row.get(4)?,
            analyte_id: row.get(5)?,
            value_text: row.get(6)?,
            units_text: row.get(7)?,
            flags_text: row.get(8)?,
            status_text: row.get(9)?,
            superseded: row.get(10)?,
            recorded_at: row.get(11)?,
        })
    })?
    .collect()
}

/// The keys of one assessment scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScopeKey {
    RunAnalyte(RunId, AnalyteId),
    Run(RunId),
    PrepBatch(PrepBatchId),
}

impl ScopeKey {
    pub(crate) fn scope(self) -> QcScope {
        match self {
            Self::RunAnalyte(..) => QcScope::RunAnalyte,
            Self::Run(_) => QcScope::Run,
            Self::PrepBatch(_) => QcScope::PrepBatch,
        }
    }

    fn columns(self) -> (Option<RunId>, Option<AnalyteId>, Option<PrepBatchId>) {
        match self {
            Self::RunAnalyte(run, analyte) => (Some(run), Some(analyte), None),
            Self::Run(run) => (Some(run), None, None),
            Self::PrepBatch(batch) => (None, None, Some(batch)),
        }
    }
}

pub(crate) struct NewAssessment<'a> {
    pub(crate) id: QcAssessmentId,
    pub(crate) key: ScopeKey,
    pub(crate) outcome: QcOutcome,
    /// The assessed batch revision, for the preparation-batch scope.
    pub(crate) prep_batch_revision: Option<i64>,
    pub(crate) supersedes: Option<QcAssessmentId>,
    pub(crate) reason: Option<&'a str>,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_assessment(
    conn: &Connection,
    new: &NewAssessment<'_>,
) -> rusqlite::Result<()> {
    let (run, analyte, batch) = new.key.columns();
    conn.prepare_cached(
        "INSERT INTO ana_qc_assessments (id, scope, run_id, analyte_id, prep_batch_id, outcome, \
         supersedes_id, reason, assessed_by, assessed_at, prep_batch_revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?
    .execute(params![
        new.id,
        new.key.scope(),
        run,
        analyte,
        batch,
        new.outcome,
        new.supersedes,
        new.reason,
        new.by,
        new.at,
        new.prep_batch_revision
    ])?;
    Ok(())
}

pub(crate) fn insert_evidence(
    conn: &Connection,
    assessment_id: QcAssessmentId,
    observation_id: QcObservationId,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_qc_assessment_evidence (assessment_id, observation_id) VALUES (?1, ?2)",
    )?
    .execute(params![assessment_id, observation_id])?;
    Ok(())
}

fn assessment_from_row(row: &Row<'_>) -> rusqlite::Result<QcAssessmentView> {
    Ok(QcAssessmentView {
        assessment_id: row.get(0)?,
        scope: row.get(1)?,
        outcome: row.get(2)?,
        supersedes_id: row.get(3)?,
        reason: row.get(4)?,
        assessed_by: row.get(5)?,
        assessed_by_display: None,
        assessed_at: row.get(6)?,
        evidence_observation_ids: Vec::new(),
    })
}

/// Every assessment of one scope, newest first: a supersession chain.
pub(crate) fn assessments(
    conn: &Connection,
    key: ScopeKey,
) -> rusqlite::Result<Vec<QcAssessmentView>> {
    let (run, analyte, batch) = key.columns();
    conn.prepare_cached(
        "SELECT id, scope, outcome, supersedes_id, reason, assessed_by, assessed_at \
         FROM ana_qc_assessments WHERE scope = ?1 AND run_id IS ?2 AND analyte_id IS ?3 \
         AND prep_batch_id IS ?4 ORDER BY assessed_at DESC, id DESC",
    )?
    .query_map(
        params![key.scope(), run, analyte, batch],
        assessment_from_row,
    )?
    .collect()
}

/// The assessment of the scope that no other supersedes; `None` means pending.
pub(crate) fn current_assessment(
    conn: &Connection,
    key: ScopeKey,
) -> rusqlite::Result<Option<QcAssessmentView>> {
    let (run, analyte, batch) = key.columns();
    conn.prepare_cached(
        "SELECT a.id, a.scope, a.outcome, a.supersedes_id, a.reason, a.assessed_by, \
         a.assessed_at FROM ana_qc_assessments a WHERE a.scope = ?1 AND a.run_id IS ?2 \
         AND a.analyte_id IS ?3 AND a.prep_batch_id IS ?4 \
         AND NOT EXISTS (SELECT 1 FROM ana_qc_assessments s WHERE s.supersedes_id = a.id)",
    )?
    .query_row(
        params![key.scope(), run, analyte, batch],
        assessment_from_row,
    )
    .optional()
}

pub(crate) fn evidence(
    conn: &Connection,
    assessment_id: QcAssessmentId,
) -> rusqlite::Result<Vec<QcObservationId>> {
    conn.prepare_cached(
        "SELECT observation_id FROM ana_qc_assessment_evidence WHERE assessment_id = ?1 \
         ORDER BY observation_id",
    )?
    .query_map([assessment_id], |row| row.get(0))?
    .collect()
}

/// The preparation-batch revision a batch assessment recorded; `None` for
/// assessments made before revisions were recorded.
pub(crate) fn assessed_prep_revision(
    conn: &Connection,
    assessment_id: QcAssessmentId,
) -> rusqlite::Result<Option<i64>> {
    conn.prepare_cached("SELECT prep_batch_revision FROM ana_qc_assessments WHERE id = ?1")?
        .query_row([assessment_id], |row| row.get(0))
}

#[cfg(test)]
mod tests;
