//! Runs, QC positions, execution failure and run closure (REQ-ANA-001,
//! REQ-ANA-006, REQ-ANA-007).

use lis_audit::AuditDetail;
use lis_catalog::MethodVersionId;
use lis_core::{Capability, InstrumentId};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{
    CODE_MAX, TEXT_MAX, actor, approved_method, clinical, instrument_name, lot_gates, reason,
};
use crate::actions;
use crate::dependency::instrument_in_service;
use crate::error::{AnalyticalError, ensure_clear, ensure_revision, gate, invalid};
use crate::model::{ExecutionId, ExecutionState, LotId, PositionId, QcRole, RunId, RunState};
use crate::queries;
use crate::repo::lots::{self, LotRow};
use crate::repo::runs::{self as repo, NewPosition, RunRow};
use crate::views::RunDetail;

/// Input of [`create_run`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreateRunInput {
    /// The instrument; must be in service.
    pub instrument_id: InstrumentId,
    /// The approved method version pinned by the accepted orders.
    pub method_version_id: MethodVersionId,
    /// The reagent, calibrator, control and internal-standard lots in use.
    pub lot_ids: Vec<LotId>,
}

/// Opens a run (REQ-ANA-001, REQ-ANA-007). Rechecked here: the instrument
/// is in service, the method version remains approved, and every lot
/// is active and unexpired. Every blocking reason is reported.
pub fn create_run(
    ctx: &mut CommandContext<'_>,
    input: CreateRunInput,
) -> Result<RunDetail, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let mut lot_ids = input.lot_ids;
    lot_ids.sort();
    lot_ids.dedup();
    instrument_name(ctx.tx(), input.instrument_id)?;
    let (_, method_gate) = approved_method(ctx.tx(), input.method_version_id)?;
    let mut gates: Vec<_> = method_gate.into_iter().collect();
    if !instrument_in_service(ctx.tx(), input.instrument_id)? {
        gates.push(gate(
            "instrument_not_in_service",
            "The instrument is not in service.",
        ));
    }
    for lot_id in &lot_ids {
        let lot = lots::lot(ctx.tx(), *lot_id)?.ok_or(AnalyticalError::NotFound("lot"))?;
        gates.extend(lot_gates(&lot, ctx.now())?);
    }
    ensure_clear(gates)?;
    let laboratory_profile_id =
        lis_identity::current_laboratory_profile(ctx.tx())?.map(|profile| profile.profile_id);
    let id = RunId::generate();
    let by = actor(ctx);
    repo::insert_run(
        ctx.tx(),
        &repo::NewRun {
            id,
            instrument_id: input.instrument_id,
            method_version_id: input.method_version_id,
            laboratory_profile_id,
            created_by: &by,
            created_at: ctx.now(),
        },
    )?;
    for lot_id in &lot_ids {
        repo::insert_run_lot(ctx.tx(), id, *lot_id)?;
    }
    ctx.audit(
        clinical(actions::CREATE_RUN)
            .record("run", id)
            .new_revision(1)
            .detail("instrument", AuditDetail::id(input.instrument_id))
            .detail("method_version", AuditDetail::id(input.method_version_id))
            .detail("lots", AuditDetail::count(lot_ids.len())),
    )?;
    queries::get_run(ctx.tx(), id, ctx.now())
}

/// Loads a run that accepts positions.
pub(crate) fn open_run(ctx: &CommandContext<'_>, id: RunId) -> Result<RunRow, AnalyticalError> {
    let run = repo::run(ctx.tx(), id)?.ok_or(AnalyticalError::NotFound("run"))?;
    if run.state != RunState::Open {
        return Err(AnalyticalError::WrongState("run"));
    }
    Ok(run)
}

/// A position label on the run: 1–64 characters, unique within the run.
pub(crate) fn position_label(value: &str) -> Result<String, AnalyticalError> {
    Ok(lis_core::required_text("positionLabel", value, CODE_MAX)?)
}

/// Input of [`add_qc_position`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AddQcPositionInput {
    /// The open run.
    pub run_id: RunId,
    /// The position label on the run.
    pub position_label: String,
    /// Control, calibrator or blank.
    pub role: QcRole,
    /// The control or calibrator lot; refused for a blank.
    #[serde(default)]
    #[ts(optional)]
    pub lot_id: Option<LotId>,
    /// A blank's sample label. Controls and calibrators use their lot's label.
    #[serde(default)]
    #[ts(optional)]
    pub sample_label: Option<String>,
}

/// Places a control, calibrator or blank on an open run (REQ-ANA-001). Such
/// positions never carry an execution, so their data never becomes a patient
/// observation. A control or calibrator needs an active, unexpired lot of the
/// matching kind and reports under that lot's sample label.
pub fn add_qc_position(
    ctx: &mut CommandContext<'_>,
    input: AddQcPositionInput,
) -> Result<RunDetail, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let label = position_label(&input.position_label)?;
    let run = open_run(ctx, input.run_id)?;
    let (_, blocked) = approved_method(ctx.tx(), run.method_version_id)?;
    ensure_clear(blocked.into_iter().collect())?;
    let (lot_id, sample_label) = qc_material(ctx, &input)?;
    if repo::position_taken(ctx.tx(), run.id, &label, &sample_label)? {
        return Err(AnalyticalError::Duplicate(
            "position or sample label on the run",
        ));
    }
    let id = PositionId::generate();
    let by = actor(ctx);
    repo::insert_position(
        ctx.tx(),
        &NewPosition {
            id,
            run_id: run.id,
            label: &label,
            role: input.role.position_role(),
            execution_id: None,
            lot_id,
            sample_label: &sample_label,
            by: &by,
            at: ctx.now(),
        },
    )?;
    let mut event = clinical(actions::ADD_QC_POSITION)
        .record("run_position", id)
        .detail("run", AuditDetail::id(run.id))
        .detail("role", AuditDetail::code(input.role.as_str()));
    if let Some(lot_id) = lot_id {
        event = event.detail("lot", AuditDetail::id(lot_id));
    }
    ctx.audit(event)?;
    queries::get_run(ctx.tx(), run.id, ctx.now())
}

/// The lot and sample label of a QC position.
fn qc_material(
    ctx: &CommandContext<'_>,
    input: &AddQcPositionInput,
) -> Result<(Option<LotId>, String), AnalyticalError> {
    let Some(kind) = input.role.lot_kind() else {
        if input.lot_id.is_some() {
            return Err(invalid("lotId", "not_applicable", "A blank has no lot."));
        }
        let label = input.sample_label.as_deref().unwrap_or_default();
        return Ok((
            None,
            lis_core::required_text("sampleLabel", label, CODE_MAX)?,
        ));
    };
    if input.sample_label.is_some() {
        return Err(invalid(
            "sampleLabel",
            "not_applicable",
            "Controls and calibrators use their lot's sample label.",
        ));
    }
    let lot_id = input
        .lot_id
        .ok_or_else(|| invalid("lotId", "required", "Choose the lot."))?;
    let lot: LotRow = lots::lot(ctx.tx(), lot_id)?.ok_or(AnalyticalError::NotFound("lot"))?;
    if lot.material_kind != kind {
        return Err(invalid(
            "lotId",
            "kind_mismatch",
            "The lot's material kind does not match the position role.",
        ));
    }
    ensure_clear(lot_gates(&lot, ctx.now())?)?;
    let label = lot.sample_label.ok_or(AnalyticalError::WrongState("lot"))?;
    Ok((Some(lot_id), label))
}

/// Input of [`fail_execution`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct FailExecutionInput {
    /// The execution.
    pub execution_id: ExecutionId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it failed, as entered.
    pub reason: String,
}

/// Marks an active execution failed, with a reason (REQ-ANA-006). The row
/// stays; a rerun links to it.
pub fn fail_execution(
    ctx: &mut CommandContext<'_>,
    input: FailExecutionInput,
) -> Result<RunDetail, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let reason = reason(&input.reason)?;
    let execution = repo::execution(ctx.tx(), input.execution_id)?
        .ok_or(AnalyticalError::NotFound("execution"))?
        .view;
    ensure_revision(execution.revision, input.expected_revision)?;
    if execution.state != ExecutionState::Active {
        return Err(AnalyticalError::WrongState("execution"));
    }
    repo::fail_execution(
        ctx.tx(),
        execution.execution_id,
        execution.revision + 1,
        &reason,
    )?;
    ctx.audit(
        clinical(actions::FAIL_EXECUTION)
            .record("execution", execution.execution_id)
            .old_revision(execution.revision)
            .new_revision(execution.revision + 1)
            .reason(reason),
    )?;
    let run_id = repo::execution_run(ctx.tx(), execution.execution_id)?
        .ok_or(AnalyticalError::NotFound("run"))?;
    queries::get_run(ctx.tx(), run_id, ctx.now())
}

/// Input of [`close_run`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CloseRunInput {
    /// The run.
    pub run_id: RunId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why, if the lab records it.
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// Closes an open run: it takes no more positions, and imports no longer
/// match its sample labels (brief §3.3).
pub fn close_run(
    ctx: &mut CommandContext<'_>,
    input: CloseRunInput,
) -> Result<RunDetail, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let reason = lis_core::optional_text("reason", input.reason.as_deref(), TEXT_MAX)?;
    let run = open_run(ctx, input.run_id)?;
    ensure_revision(run.revision, input.expected_revision)?;
    let by = actor(ctx);
    repo::close_run(
        ctx.tx(),
        run.id,
        run.revision + 1,
        reason.as_deref(),
        &by,
        ctx.now(),
    )?;
    let mut event = clinical(actions::CLOSE_RUN)
        .record("run", run.id)
        .old_revision(run.revision)
        .new_revision(run.revision + 1);
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    queries::get_run(ctx.tx(), run.id, ctx.now())
}
