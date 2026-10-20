//! Patient positions: placing accepted material on a run creates its
//! execution and links it to the order components the run's method serves
//! (REQ-ANA-001, REQ-ANA-006, REQ-SPC-003, REQ-ORD-002).

use lis_audit::AuditDetail;
use lis_core::{Capability, Decimal, DecimalText, GateReason};
use lis_orders::ItemKind;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::runs::{open_run, position_label};
use super::{TEXT_MAX, actor, approved_method, clinical};
use crate::actions;
use crate::error::{AnalyticalError, ensure_clear, gate, invalid};
use crate::model::{
    AttemptKind, ExecutionId, PositionId, PositionRole, PrepBatchId, RegistrationState, RunId,
};
use crate::queries;
use crate::repo::published::{self, ComponentRef, ContainerRef};
use crate::repo::runs::{self as repo, NewExecution, NewPosition, RunRow};
use crate::repo::{lots, readiness};
use crate::served::{CatalogCache, served_components};
use crate::views::RunDetail;

/// Input of [`add_patient_position`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AddPatientPositionInput {
    /// The open run.
    pub run_id: RunId,
    /// The position label on the run.
    pub position_label: String,
    /// The scanned container label code; it becomes the position's sample label.
    pub container_label: String,
    /// Initial, rerun, re-extraction or dilution.
    pub attempt_kind: AttemptKind,
    /// The execution a rerun, re-extraction or dilution follows.
    #[serde(default)]
    #[ts(optional)]
    pub related_execution_id: Option<ExecutionId>,
    /// The dilution factor, recorded only: the source must report final concentration.
    #[serde(default)]
    #[ts(optional)]
    pub dilution_factor: Option<DecimalText>,
    /// The preparation batch, for the run's method version.
    #[serde(default)]
    #[ts(optional)]
    pub prep_batch_id: Option<PrepBatchId>,
    /// Why: required for a follow-on attempt and for add-on tests (R10).
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// The validated placement: the position label and the execution's placement.
struct Placement {
    label: String,
    kind: AttemptKind,
    related: Option<ExecutionId>,
    dilution_factor: Option<DecimalText>,
    prep_batch_id: Option<PrepBatchId>,
    reason: Option<String>,
}

/// Places accepted patient material on an open run (REQ-ANA-001). Rechecked
/// in this transaction, in order: the container's specimen gate against the
/// method's specimen type, then the order gate of every component, no active execution
/// for an initial attempt, and a dilution only from a source that reports
/// final concentration. Copies the container's printed label as the sample label.
pub fn add_patient_position(
    ctx: &mut CommandContext<'_>,
    input: AddPatientPositionInput,
) -> Result<RunDetail, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let placement = validate(&input)?;
    let run = open_run(ctx, input.run_id)?;
    let container = published::container_by_label(ctx.tx(), input.container_label.trim())?
        .ok_or(AnalyticalError::NotFound("container"))?;
    let (method, blocked) = approved_method(ctx.tx(), run.method_version_id)?;
    ensure_clear(blocked.into_iter().collect())?;
    // REQ-SPC-003, REQ-SPC-005: the material itself is rechecked first.
    let specimen =
        lis_specimens::testing_gate(ctx.tx(), container.container_id, &method.specimen_type_code)?;
    ensure_clear(specimen.err().unwrap_or_default())?;
    let components = components_to_test(ctx, &run, &container, &placement)?;
    ensure_clear(order_gates(ctx, &run, &components, placement.kind)?)?;
    // R10: stability for add-ons is not computed; the analyst attests it.
    let add_on = components.iter().any(|c| c.item_kind == ItemKind::AddOn);
    if add_on && placement.reason.is_none() {
        return Err(invalid(
            "reason",
            "add_on_reason_required",
            "State that the add-on material was checked under the stability SOP.",
        ));
    }
    check_prep_batch(ctx, &run, placement.prep_batch_id)?;
    if repo::position_taken(ctx.tx(), run.id, &placement.label, &container.label_code)? {
        return Err(AnalyticalError::Duplicate(
            "position or sample label on the run",
        ));
    }
    let position = record(ctx, &run, &container, &components, &placement)?;
    let mut event = clinical(actions::ADD_PATIENT_POSITION)
        .record("run_position", position.0)
        .detail("run", AuditDetail::id(run.id))
        .detail("execution", AuditDetail::id(position.1))
        .detail("container", AuditDetail::id(container.container_id))
        .detail("attempt_kind", AuditDetail::code(placement.kind.as_str()))
        .detail("components", AuditDetail::count(components.len()));
    if let Some(reason) = placement.reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    queries::get_run(ctx.tx(), run.id, ctx.now())
}

/// Validates the label, and the attempt kind against its lineage, dilution
/// factor and reason.
fn validate(input: &AddPatientPositionInput) -> Result<Placement, AnalyticalError> {
    let label = position_label(&input.position_label)?;
    let reason = lis_core::optional_text("reason", input.reason.as_deref(), TEXT_MAX)?;
    let follow_on = input.attempt_kind != AttemptKind::Initial;
    match (follow_on, input.related_execution_id, &reason) {
        (false, Some(_), _) => {
            return Err(invalid(
                "relatedExecutionId",
                "not_applicable",
                "An initial attempt follows no execution.",
            ));
        }
        (true, None, _) => {
            return Err(invalid(
                "relatedExecutionId",
                "required",
                "Choose the execution this attempt follows.",
            ));
        }
        (true, Some(_), None) => {
            return Err(invalid(
                "reason",
                "required",
                "Enter why this attempt is made.",
            ));
        }
        (false, None, _) | (true, Some(_), Some(_)) => {}
    }
    let dilution = input.attempt_kind == AttemptKind::Dilution;
    match (&input.dilution_factor, dilution) {
        (Some(factor), true) if factor.to_decimal() > Decimal::ZERO => {}
        (Some(_), true) => {
            return Err(invalid(
                "dilutionFactor",
                "not_positive",
                "Enter a positive factor.",
            ));
        }
        (None, true) => {
            return Err(invalid(
                "dilutionFactor",
                "required",
                "Enter the dilution factor.",
            ));
        }
        (Some(_), false) => {
            return Err(invalid(
                "dilutionFactor",
                "not_applicable",
                "Only a dilution has a dilution factor.",
            ));
        }
        (None, false) => {}
    }
    Ok(Placement {
        label,
        kind: input.attempt_kind,
        related: input.related_execution_id,
        dilution_factor: input.dilution_factor.clone(),
        prep_batch_id: input.prep_batch_id,
        reason,
    })
}

/// The components the new execution serves. An initial attempt takes the
/// served, active components that no active execution covers yet; a
/// follow-on attempt takes its related execution's components, on the same
/// specimen and method version.
fn components_to_test(
    ctx: &CommandContext<'_>,
    run: &RunRow,
    container: &ContainerRef,
    placement: &Placement,
) -> Result<Vec<ComponentRef>, AnalyticalError> {
    let Some(related) = placement.related else {
        return initial_components(ctx, run, container);
    };
    let related =
        repo::execution(ctx.tx(), related)?.ok_or(AnalyticalError::NotFound("execution"))?;
    if related.method_version_id != run.method_version_id {
        return Err(invalid(
            "relatedExecutionId",
            "method_mismatch",
            "The related execution used another method version.",
        ));
    }
    let related_container = published::container(ctx.tx(), related.view.container_id)?
        .ok_or(AnalyticalError::NotFound("container"))?;
    if related_container.specimen_id != container.specimen_id {
        return Err(invalid(
            "relatedExecutionId",
            "different_specimen",
            "The related execution tested another specimen.",
        ));
    }
    let mut components = Vec::new();
    for id in repo::execution_components(ctx.tx(), related.view.execution_id)? {
        components.push(
            published::component(ctx.tx(), id)?
                .ok_or(AnalyticalError::NotFound("order component"))?,
        );
    }
    Ok(components)
}

/// The served, active components no active execution covers yet. Refused
/// when every such component is already in progress, or none exists.
fn initial_components(
    ctx: &CommandContext<'_>,
    run: &RunRow,
    container: &ContainerRef,
) -> Result<Vec<ComponentRef>, AnalyticalError> {
    let mut cache = CatalogCache::default();
    let (_, served) = served_components(
        ctx.tx(),
        &mut cache,
        container.accession_id,
        run.method_version_id,
    )?;
    let mut pending = Vec::new();
    let mut in_progress = false;
    for component in served.into_iter().map(|s| s.component).filter(|c| c.active) {
        if repo::component_in_progress(ctx.tx(), component.component_id)? {
            in_progress = true;
        } else {
            pending.push(component);
        }
    }
    match (pending.is_empty(), in_progress) {
        (false, _) => Ok(pending),
        (true, true) => Err(AnalyticalError::AlreadyInProgress),
        (true, false) => Err(AnalyticalError::NothingToTest),
    }
}

/// Every blocking gate of the components, each code once: each component's
/// order gate (REQ-ORD-002) and the dilution rule.
fn order_gates(
    ctx: &CommandContext<'_>,
    run: &RunRow,
    components: &[ComponentRef],
    kind: AttemptKind,
) -> Result<Vec<GateReason>, AnalyticalError> {
    let mut found = Vec::new();
    for component in components {
        if let Err(order_gates) = lis_orders::testing_gate(ctx.tx(), component.component_id)? {
            found.extend(order_gates);
        }
    }
    if kind == AttemptKind::Dilution && !reports_final_concentration(ctx, run)? {
        found.push(gate(
            "dilution_not_supported",
            "The instrument's source registration does not report final concentrations.",
        ));
    }
    let mut unique: Vec<GateReason> = Vec::new();
    for reason in found {
        if !unique.iter().any(|seen| seen.code == reason.code) {
            unique.push(reason);
        }
    }
    Ok(unique)
}

/// REQ-ANA-006: the LIS does no dilution arithmetic, so a dilution needs a
/// source that reports the final concentration.
fn reports_final_concentration(
    ctx: &CommandContext<'_>,
    run: &RunRow,
) -> Result<bool, AnalyticalError> {
    let active =
        readiness::registrations(ctx.tx(), run.instrument_id, Some(RegistrationState::Active))?;
    Ok(active.iter().any(|r| r.reports_final_concentration))
}

/// A preparation batch must exist and prepare for the run's method version.
fn check_prep_batch(
    ctx: &CommandContext<'_>,
    run: &RunRow,
    batch: Option<PrepBatchId>,
) -> Result<(), AnalyticalError> {
    let Some(batch) = batch else {
        return Ok(());
    };
    let batch =
        lots::prep_batch(ctx.tx(), batch)?.ok_or(AnalyticalError::NotFound("preparation batch"))?;
    if batch.method_version_id != run.method_version_id {
        return Err(invalid(
            "prepBatchId",
            "method_mismatch",
            "The preparation batch is for another method version.",
        ));
    }
    Ok(())
}

/// Writes the execution, its components and the position.
fn record(
    ctx: &CommandContext<'_>,
    run: &RunRow,
    container: &ContainerRef,
    components: &[ComponentRef],
    placement: &Placement,
) -> Result<(PositionId, ExecutionId), AnalyticalError> {
    let execution_id = ExecutionId::generate();
    let position_id = PositionId::generate();
    let by = actor(ctx);
    repo::insert_execution(
        ctx.tx(),
        &NewExecution {
            id: execution_id,
            container_id: container.container_id,
            method_version_id: run.method_version_id,
            attempt_kind: placement.kind,
            related_execution_id: placement.related,
            dilution_factor: placement.dilution_factor.as_ref(),
            prep_batch_id: placement.prep_batch_id,
            reason: placement.reason.as_deref(),
            by: &by,
            at: ctx.now(),
        },
    )?;
    for component in components {
        repo::insert_execution_component(ctx.tx(), execution_id, component.component_id)?;
    }
    repo::insert_position(
        ctx.tx(),
        &NewPosition {
            id: position_id,
            run_id: run.id,
            label: &placement.label,
            role: PositionRole::Patient,
            execution_id: Some(execution_id),
            lot_id: None,
            sample_label: &container.label_code,
            by: &by,
            at: ctx.now(),
        },
    )?;
    Ok((position_id, execution_id))
}
