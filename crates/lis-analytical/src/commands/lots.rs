//! Lots and preparation batches, open → completed (REQ-ANA-001, REQ-ANA-007).

use lis_audit::AuditDetail;
use lis_catalog::MethodVersionId;
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{CODE_MAX, NAME_MAX, TEXT_MAX, actor, approved_method, clinical, lot_gates, reason};
use crate::actions;
use crate::error::{AnalyticalError, ensure_clear, ensure_revision, gate, invalid};
use crate::model::{CivilDate, LotId, LotState, MaterialKind, PrepBatchId, PrepBatchState};
use crate::queries;
use crate::repo::lots::{self as repo, NewLot, NewStep};
use crate::views::{LotView, PrepBatchView};

/// Input of [`register_lot`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterLotInput {
    /// Reagent, calibrator, control or internal standard.
    pub material_kind: MaterialKind,
    /// The product name.
    pub product_name: String,
    /// The manufacturer's lot number.
    pub lot_number: String,
    /// The level, as labelled.
    #[serde(default)]
    #[ts(optional)]
    pub level_text: Option<String>,
    /// The lab-defined sample label the analyzer reports for this control or
    /// calibrator; required for those kinds and refused for others.
    #[serde(default)]
    #[ts(optional)]
    pub sample_label: Option<String>,
    /// The last day of use, `YYYY-MM-DD`.
    pub expires_on: String,
}

/// Records a lot (REQ-ANA-007). Expiry is derived from `expiresOn`, never stored.
pub fn register_lot(
    ctx: &mut CommandContext<'_>,
    input: RegisterLotInput,
) -> Result<LotView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let product_name = lis_core::required_text("productName", &input.product_name, NAME_MAX)?;
    let lot_number = lis_core::required_text("lotNumber", &input.lot_number, CODE_MAX)?;
    let level_text = lis_core::optional_text("levelText", input.level_text.as_deref(), CODE_MAX)?;
    let sample_label = sample_label(input.material_kind, input.sample_label.as_deref())?;
    let expires_on = CivilDate::parse("expiresOn", &input.expires_on)?;
    if repo::lot_exists(ctx.tx(), input.material_kind, &product_name, &lot_number)? {
        return Err(AnalyticalError::Duplicate("lot"));
    }
    let id = LotId::generate();
    let by = actor(ctx);
    repo::insert_lot(
        ctx.tx(),
        &NewLot {
            id,
            material_kind: input.material_kind,
            product_name: &product_name,
            lot_number: &lot_number,
            level_text: level_text.as_deref(),
            sample_label: sample_label.as_deref(),
            expires_on,
            by: &by,
            at: ctx.now(),
        },
    )?;
    ctx.audit(
        clinical(actions::REGISTER_LOT)
            .record("lot", id)
            .new_revision(1)
            .detail(
                "material_kind",
                AuditDetail::code(input.material_kind.as_str()),
            ),
    )?;
    queries::lot_view(ctx.tx(), id, ctx.now())
}

/// Controls and calibrators carry the label the analyzer reports; other kinds do not.
fn sample_label(
    kind: MaterialKind,
    value: Option<&str>,
) -> Result<Option<String>, AnalyticalError> {
    let label = lis_core::optional_text("sampleLabel", value, CODE_MAX)?;
    match (kind, label) {
        (MaterialKind::Control | MaterialKind::Calibrator, Some(label)) => Ok(Some(label)),
        (MaterialKind::Control | MaterialKind::Calibrator, None) => Err(invalid(
            "sampleLabel",
            "required",
            "Enter the sample label the analyzer reports for this material.",
        )),
        (MaterialKind::Reagent | MaterialKind::InternalStandard, None) => Ok(None),
        (MaterialKind::Reagent | MaterialKind::InternalStandard, Some(_)) => Err(invalid(
            "sampleLabel",
            "not_applicable",
            "Only controls and calibrators have a sample label.",
        )),
    }
}

/// Input of [`withdraw_lot`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct WithdrawLotInput {
    /// The lot.
    pub lot_id: LotId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why, as entered.
    pub reason: String,
}

/// Withdraws an active lot from new use (REQ-ANA-007). Work already done
/// keeps its link; a deviation on the lot is how past work is questioned.
pub fn withdraw_lot(
    ctx: &mut CommandContext<'_>,
    input: WithdrawLotInput,
) -> Result<LotView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let reason = reason(&input.reason)?;
    let lot = repo::lot(ctx.tx(), input.lot_id)?.ok_or(AnalyticalError::NotFound("lot"))?;
    ensure_revision(lot.revision, input.expected_revision)?;
    if lot.state != LotState::Active {
        return Err(AnalyticalError::WrongState("lot"));
    }
    let by = actor(ctx);
    repo::withdraw_lot(ctx.tx(), lot.id, lot.revision + 1, &reason, &by, ctx.now())?;
    ctx.audit(
        clinical(actions::WITHDRAW_LOT)
            .record("lot", lot.id)
            .old_revision(lot.revision)
            .new_revision(lot.revision + 1)
            .reason(reason),
    )?;
    queries::lot_view(ctx.tx(), lot.id, ctx.now())
}

/// Input of [`create_prep_batch`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreatePrepBatchInput {
    /// The approved method version the batch prepares for, including pinned superseded work.
    pub method_version_id: MethodVersionId,
}

/// Opens a preparation batch for an approved method version (REQ-ANA-001).
pub fn create_prep_batch(
    ctx: &mut CommandContext<'_>,
    input: CreatePrepBatchInput,
) -> Result<PrepBatchView, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let (_, blocked) = approved_method(ctx.tx(), input.method_version_id)?;
    ensure_clear(blocked.into_iter().collect())?;
    let id = PrepBatchId::generate();
    let by = actor(ctx);
    repo::insert_prep_batch(ctx.tx(), id, input.method_version_id, &by, ctx.now())?;
    ctx.audit(
        clinical(actions::CREATE_PREP_BATCH)
            .record("prep_batch", id)
            .new_revision(1)
            .detail("method_version", AuditDetail::id(input.method_version_id)),
    )?;
    queries::prep_batch(ctx.tx(), id)
}

/// Input of [`record_prep_step`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RecordPrepStepInput {
    /// The batch.
    pub prep_batch_id: PrepBatchId,
    /// The lab-defined step code.
    pub step_code: String,
    /// The lot used, if any; must be active and unexpired.
    #[serde(default)]
    #[ts(optional)]
    pub lot_id: Option<LotId>,
    /// A note, as entered.
    #[serde(default)]
    #[ts(optional)]
    pub note: Option<String>,
}

/// Appends a step to an open preparation batch and raises its revision
/// (REQ-ANA-001). A completed batch takes no more steps. A lot the step uses
/// is rechecked for withdrawal and expiry in this transaction.
pub fn record_prep_step(
    ctx: &mut CommandContext<'_>,
    input: RecordPrepStepInput,
) -> Result<PrepBatchView, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let step_code = lis_core::required_text("stepCode", &input.step_code, CODE_MAX)?;
    let note = lis_core::optional_text("note", input.note.as_deref(), TEXT_MAX)?;
    let batch = open_batch(ctx, input.prep_batch_id)?;
    if let Some(lot_id) = input.lot_id {
        let lot = repo::lot(ctx.tx(), lot_id)?.ok_or(AnalyticalError::NotFound("lot"))?;
        ensure_clear(lot_gates(&lot, ctx.now())?)?;
    }
    let by = actor(ctx);
    let step = repo::insert_prep_step(
        ctx.tx(),
        &NewStep {
            batch_id: input.prep_batch_id,
            step_code: &step_code,
            lot_id: input.lot_id,
            note: note.as_deref(),
            by: &by,
            at: ctx.now(),
        },
    )?;
    repo::touch_prep_batch(ctx.tx(), batch.prep_batch_id, batch.revision + 1)?;
    let mut event = clinical(actions::RECORD_PREP_STEP)
        .record("prep_batch", input.prep_batch_id)
        .old_revision(batch.revision)
        .new_revision(batch.revision + 1)
        .detail("prep_step", AuditDetail::id(step));
    if let Some(lot_id) = input.lot_id {
        event = event.detail("lot", AuditDetail::id(lot_id));
    }
    ctx.audit(event)?;
    queries::prep_batch(ctx.tx(), input.prep_batch_id)
}

/// Loads a preparation batch that still takes steps.
fn open_batch(ctx: &CommandContext<'_>, id: PrepBatchId) -> Result<PrepBatchView, AnalyticalError> {
    let batch =
        repo::prep_batch(ctx.tx(), id)?.ok_or(AnalyticalError::NotFound("preparation batch"))?;
    if batch.state != PrepBatchState::Open {
        return Err(AnalyticalError::WrongState("preparation batch"));
    }
    Ok(batch)
}

/// Input of [`complete_prep_batch`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CompletePrepBatchInput {
    /// The batch.
    pub prep_batch_id: PrepBatchId,
    /// The revision shown: the completer confirms exactly the steps seen.
    #[ts(type = "number")]
    pub expected_revision: i64,
}

/// Completes an open preparation batch with at least one recorded step
/// (REQ-ANA-001). A completed batch never changes, so a batch assessment,
/// which records the revision, and a review bound to it stay true to the
/// steps assessed.
pub fn complete_prep_batch(
    ctx: &mut CommandContext<'_>,
    input: CompletePrepBatchInput,
) -> Result<PrepBatchView, AnalyticalError> {
    ctx.require(Capability::PerformAnalysis)?;
    let batch = open_batch(ctx, input.prep_batch_id)?;
    ensure_revision(batch.revision, input.expected_revision)?;
    if repo::prep_steps(ctx.tx(), batch.prep_batch_id)?.is_empty() {
        return Err(AnalyticalError::GateBlocked(vec![gate(
            "prep_no_steps",
            "Record the preparation steps before completing the batch.",
        )]));
    }
    let by = actor(ctx);
    repo::complete_prep_batch(
        ctx.tx(),
        batch.prep_batch_id,
        batch.revision + 1,
        &by,
        ctx.now(),
    )?;
    ctx.audit(
        clinical(actions::COMPLETE_PREP_BATCH)
            .record("prep_batch", batch.prep_batch_id)
            .old_revision(batch.revision)
            .new_revision(batch.revision + 1),
    )?;
    queries::prep_batch(ctx.tx(), batch.prep_batch_id)
}
