//! QC observations from imports and QC assessments (REQ-ANA-003 re-export
//! part for QC rows, REQ-ANA-004, REQ-ANA-005).

use std::collections::BTreeSet;

use lis_audit::AuditDetail;
use lis_catalog::AnalyteId;
use lis_core::Capability;
use lis_interfaces::ReceiptId;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{TEXT_MAX, clinical, require_user};
use crate::actions;
use crate::error::{AnalyticalError, gate, invalid};
use crate::model::{
    PositionId, PositionRole, PrepBatchId, PrepBatchState, QcAssessmentId, QcObservationId,
    QcOutcome, QcScope, RevisionIndicator, RunId, RunState,
};
use crate::queries;
use crate::reexport::{RowDecision, SourceRow, decide};
use crate::repo::qc::{self as repo, NewAssessment, NewObservation, ScopeKey};
use crate::repo::{conflicts, lots, runs};
use crate::views::{QcAssessmentView, SourceRegistrationView};

/// One control, calibrator or blank row of an import (brief §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QcRow {
    /// The matched position; its role must not be `patient`.
    pub position_id: PositionId,
    /// The durable receipt the row came from.
    pub receipt_id: ReceiptId,
    /// Position in the message, `P<n>/O<n>/R<n>`.
    pub record_path: String,
    /// The source test code, as received.
    pub source_test_code: String,
    /// The compared content, as received.
    pub row: SourceRow,
}

/// Records QC rows of an import (brief §3.4): stored only in
/// `ana_qc_observations`, never as patient observations. Each row is decided
/// with [`decide`](crate::reexport::decide) against the current observation
/// of its position and test code: new rows and declared corrections are
/// stored (a correction supersedes, so the run × analyte assessment becomes
/// `qc_stale`); duplicates store nothing. A conflict is kept here as a QC
/// conflict against the observation it contradicts: it blocks the run ×
/// analyte (`qc_conflict`) until a person resolves it, so the caller does
/// not quarantine it again. Returns one decision per row, in order. Not an
/// IPC command; the importer calls it with `instrument_intake`.
pub fn record_qc_observations(
    ctx: &mut CommandContext<'_>,
    rows: &[QcRow],
) -> Result<Vec<RowDecision>, AnalyticalError> {
    ctx.require(Capability::InstrumentIntake)?;
    let mut decisions = Vec::with_capacity(rows.len());
    for row in rows {
        decisions.push(record_one(ctx, row)?);
    }
    Ok(decisions)
}

fn record_one(ctx: &CommandContext<'_>, row: &QcRow) -> Result<RowDecision, AnalyticalError> {
    let position =
        runs::position(ctx.tx(), row.position_id)?.ok_or(AnalyticalError::NotFound("position"))?;
    // Structural separation: the LIS-set role decides the route, never the payload.
    if position.role == PositionRole::Patient {
        return Err(AnalyticalError::WrongState("position"));
    }
    if position.run_state != RunState::Open {
        return Err(AnalyticalError::WrongState("run"));
    }
    let registration = queries::active_registration(ctx.tx(), position.instrument_id)?;
    let indicator = registration
        .as_ref()
        .map_or(RevisionIndicator::None, |r| r.revision_indicator);
    let current = repo::current_observation(ctx.tx(), position.id, &row.source_test_code)?;
    let decision = decide(
        current.as_ref().map(|(_, stored)| stored),
        &row.row,
        indicator,
    );
    let mut new = NewObservation {
        position_id: position.id,
        receipt_id: row.receipt_id,
        record_path: &row.record_path,
        source_test_code: &row.source_test_code,
        analyte_id: mapped_analyte(registration.as_ref(), &row.source_test_code),
        row: &row.row,
        supersedes: None,
        at: ctx.now(),
    };
    match (decision, current) {
        (RowDecision::Duplicate, _) => return Ok(decision),
        (RowDecision::Conflict, Some((contradicted, _))) => {
            record_conflict(ctx, &new, contradicted)?;
            return Ok(decision);
        }
        (RowDecision::Correction, Some((superseded, _))) => new.supersedes = Some(superseded),
        (RowDecision::New, _) => {}
        // `decide` classifies a row as changed only against a stored one.
        (RowDecision::Correction | RowDecision::Conflict, None) => {
            return Err(AnalyticalError::WrongState("QC observation"));
        }
    }
    let id = repo::insert_observation(ctx.tx(), &new)?;
    let mut event = clinical(actions::RECORD_QC_OBSERVATIONS)
        .record("qc_observation", id)
        .detail("position", AuditDetail::id(position.id))
        .detail("receipt", AuditDetail::id(row.receipt_id));
    if let Some(superseded) = new.supersedes {
        event = event.detail("supersedes", AuditDetail::id(superseded));
    }
    ctx.audit(event)?;
    Ok(decision)
}

/// Keeps a changed row the registration cannot classify against the
/// observation it contradicts, so the run × analyte stays blocked until a
/// person decides ([`resolve_qc_conflict`](crate::resolve_qc_conflict)). An
/// identical row already awaiting a decision is not recorded twice.
fn record_conflict(
    ctx: &CommandContext<'_>,
    new: &NewObservation<'_>,
    contradicted: QcObservationId,
) -> Result<(), AnalyticalError> {
    let Some(id) = conflicts::insert(ctx.tx(), new, contradicted)? else {
        return Ok(());
    };
    ctx.audit(
        clinical(actions::RECORD_QC_OBSERVATIONS)
            .record("qc_conflict", id)
            .detail("position", AuditDetail::id(new.position_id))
            .detail("receipt", AuditDetail::id(new.receipt_id))
            .detail("conflicts_with", AuditDetail::id(contradicted)),
    )?;
    Ok(())
}

/// The analyte the registration maps `code` to, exactly as received.
fn mapped_analyte(registration: Option<&SourceRegistrationView>, code: &str) -> Option<AnalyteId> {
    registration?
        .test_codes
        .iter()
        .find(|mapped| mapped.source_test_code == code)
        .map(|mapped| mapped.analyte_id)
}

/// Input of [`assess_qc`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AssessQcInput {
    /// Run × analyte, run or preparation batch.
    pub scope: QcScope,
    /// The run, for the run and run × analyte scopes.
    #[serde(default)]
    #[ts(optional)]
    pub run_id: Option<RunId>,
    /// The analyte, for the run × analyte scope.
    #[serde(default)]
    #[ts(optional)]
    pub analyte_id: Option<AnalyteId>,
    /// The batch, for the preparation-batch scope.
    #[serde(default)]
    #[ts(optional)]
    pub prep_batch_id: Option<PrepBatchId>,
    /// Accepted or rejected; there is no waiver.
    pub outcome: QcOutcome,
    /// The observations relied on. For run × analyte: exactly the current
    /// observations shown.
    pub evidence_observation_ids: Vec<QcObservationId>,
    /// The current assessment of the scope, when there is one.
    #[serde(default)]
    #[ts(optional)]
    pub supersedes_assessment_id: Option<QcAssessmentId>,
    /// Why, as entered; required when superseding.
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// Records a QC decision by a signed-in user (REQ-ANA-004, REQ-ANA-005).
/// Assessments are append-only: a new one supersedes the current one of the
/// same scope with a reason, and a rejection is never superseded by an
/// acceptance. A run × analyte assessment cites exactly the current
/// observations, and an acceptance needs at least one; later QC data makes
/// it stale.
pub fn assess_qc(
    ctx: &mut CommandContext<'_>,
    input: AssessQcInput,
) -> Result<QcAssessmentView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let by = require_user(ctx)?;
    let key = scope_key(ctx, &input)?;
    let reason = lis_core::optional_text("reason", input.reason.as_deref(), TEXT_MAX)?;
    let current = repo::current_assessment(ctx.tx(), key)?;
    if input.supersedes_assessment_id != current.as_ref().map(|c| c.assessment_id) {
        return Err(AnalyticalError::RevisionConflict);
    }
    if current.is_some() && reason.is_none() {
        return Err(invalid(
            "reason",
            "required",
            "Enter why the assessment changes.",
        ));
    }
    // REQ-ANA-005: no waiver path; failed QC stays failed.
    let relabel = current.as_ref().map(|c| c.outcome) == Some(QcOutcome::Rejected);
    if relabel && input.outcome == QcOutcome::Accepted {
        return Err(AnalyticalError::RejectionFinal);
    }
    let evidence = evidence(ctx, key, input.outcome, &input.evidence_observation_ids)?;
    let prep_batch_revision = prep_revision(ctx, key, input.outcome)?;
    let id = QcAssessmentId::generate();
    repo::insert_assessment(
        ctx.tx(),
        &NewAssessment {
            id,
            key,
            outcome: input.outcome,
            prep_batch_revision,
            supersedes: input.supersedes_assessment_id,
            reason: reason.as_deref(),
            by: &by,
            at: ctx.now(),
        },
    )?;
    for observation in &evidence {
        repo::insert_evidence(ctx.tx(), id, *observation)?;
    }
    let mut event = clinical(actions::ASSESS_QC)
        .record("qc_assessment", id)
        .detail("scope", AuditDetail::code(key.scope().as_str()))
        .detail("outcome", AuditDetail::code(input.outcome.as_str()))
        .detail("evidence", AuditDetail::count(evidence.len()));
    if let Some(revision) = prep_batch_revision {
        event = event.detail("prep_batch_revision", AuditDetail::integer(revision));
    }
    if let Some(superseded) = input.supersedes_assessment_id {
        event = event.detail("supersedes", AuditDetail::id(superseded));
    }
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    queries::qc_assessment(ctx.tx(), key, id)
}

/// The scope's keys, each required where the scope needs it and refused
/// where it does not; the run, analyte or batch must exist.
fn scope_key(ctx: &CommandContext<'_>, input: &AssessQcInput) -> Result<ScopeKey, AnalyticalError> {
    let key = match (
        input.scope,
        input.run_id,
        input.analyte_id,
        input.prep_batch_id,
    ) {
        (QcScope::RunAnalyte, Some(run), Some(analyte), None) => ScopeKey::RunAnalyte(run, analyte),
        (QcScope::Run, Some(run), None, None) => ScopeKey::Run(run),
        (QcScope::PrepBatch, None, None, Some(batch)) => ScopeKey::PrepBatch(batch),
        (QcScope::RunAnalyte | QcScope::Run | QcScope::PrepBatch, ..) => {
            return Err(invalid(
                "scope",
                "scope_fields",
                "Give exactly the run, analyte or preparation batch the scope needs.",
            ));
        }
    };
    match key {
        ScopeKey::RunAnalyte(run, analyte) => {
            let run = runs::run(ctx.tx(), run)?.ok_or(AnalyticalError::NotFound("run"))?;
            let method = lis_catalog::get_method_version(ctx.tx(), run.method_version_id)?;
            if !method
                .analytes
                .iter()
                .any(|a| a.limits.analyte_id == analyte)
            {
                return Err(AnalyticalError::NotFound("analyte"));
            }
        }
        ScopeKey::Run(run) => {
            runs::run(ctx.tx(), run)?.ok_or(AnalyticalError::NotFound("run"))?;
        }
        ScopeKey::PrepBatch(batch) => {
            lots::prep_batch(ctx.tx(), batch)?
                .ok_or(AnalyticalError::NotFound("preparation batch"))?;
        }
    }
    Ok(key)
}

/// For a preparation batch, the revision assessed: the batch's recorded
/// steps are its evidence, and only a completed (frozen) batch can be
/// accepted (REQ-ANA-001, REQ-ANA-004). A rejection may stop an open batch.
fn prep_revision(
    ctx: &CommandContext<'_>,
    key: ScopeKey,
    outcome: QcOutcome,
) -> Result<Option<i64>, AnalyticalError> {
    let ScopeKey::PrepBatch(batch) = key else {
        return Ok(None);
    };
    let batch =
        lots::prep_batch(ctx.tx(), batch)?.ok_or(AnalyticalError::NotFound("preparation batch"))?;
    if outcome == QcOutcome::Accepted && batch.state != PrepBatchState::Completed {
        return Err(AnalyticalError::GateBlocked(vec![gate(
            "prep_not_completed",
            "Complete the preparation batch before accepting it.",
        )]));
    }
    Ok(Some(batch.revision))
}

/// The cited observations, checked against the scope.
fn evidence(
    ctx: &CommandContext<'_>,
    key: ScopeKey,
    outcome: QcOutcome,
    cited: &[QcObservationId],
) -> Result<Vec<QcObservationId>, AnalyticalError> {
    let cited: BTreeSet<QcObservationId> = cited.iter().copied().collect();
    let scope_run = match key {
        ScopeKey::RunAnalyte(run, _) | ScopeKey::Run(run) => run,
        // No QC observation belongs to a preparation batch: control,
        // calibrator and blank positions carry no execution.
        ScopeKey::PrepBatch(_) if cited.is_empty() => return Ok(Vec::new()),
        ScopeKey::PrepBatch(_) => {
            return Err(invalid(
                "evidenceObservationIds",
                "not_applicable",
                "A preparation batch is assessed on its recorded steps; cite QC observations on the run.",
            ));
        }
    };
    for observation in &cited {
        let run = repo::observation_run(ctx.tx(), *observation)?
            .ok_or(AnalyticalError::NotFound("QC observation"))?;
        if run != scope_run {
            return Err(invalid(
                "evidenceObservationIds",
                "other_run",
                "Cite only observations of this run.",
            ));
        }
    }
    if let ScopeKey::RunAnalyte(run, analyte) = key {
        let current: BTreeSet<QcObservationId> =
            repo::current_run_analyte_observations(ctx.tx(), run, analyte)?
                .into_iter()
                .collect();
        if current != cited {
            return Err(AnalyticalError::EvidenceChanged);
        }
        if outcome == QcOutcome::Accepted && cited.is_empty() {
            return Err(invalid(
                "evidenceObservationIds",
                "required",
                "QC can be accepted only on received QC observations.",
            ));
        }
    }
    Ok(cited.into_iter().collect())
}
