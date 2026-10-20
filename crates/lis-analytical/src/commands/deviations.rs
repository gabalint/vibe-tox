//! Deviations (REQ-ANA-004, REQ-ANA-005): while open, they block every
//! result that depends on their target.

use lis_audit::AuditDetail;
use lis_catalog::AnalyteId;
use lis_core::{Capability, InstrumentId};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{TEXT_MAX, actor, clinical, instrument_name, reason};
use crate::actions;
use crate::error::{AnalyticalError, ensure_revision};
use crate::model::{
    DeviationId, DeviationScope, DeviationState, LotId, PositionId, PrepBatchId, RunId,
};
use crate::repo::deviations::{self as repo, Closing, NewDeviation};
use crate::repo::{lots, runs};
use crate::views::DeviationView;

/// What a deviation applies to.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
#[ts(export)]
pub enum DeviationTarget {
    /// An instrument.
    Instrument(InstrumentId),
    /// A lot.
    Lot(LotId),
    /// A preparation batch.
    PrepBatch(PrepBatchId),
    /// A run.
    Run(RunId),
    /// One run position.
    Position(PositionId),
}

impl DeviationTarget {
    fn scope(self) -> DeviationScope {
        match self {
            Self::Instrument(_) => DeviationScope::Instrument,
            Self::Lot(_) => DeviationScope::Lot,
            Self::PrepBatch(_) => DeviationScope::PrepBatch,
            Self::Run(_) => DeviationScope::Run,
            Self::Position(_) => DeviationScope::Position,
        }
    }

    /// Refuses a target that does not exist; returns its stored ID text and audit detail.
    fn check(self, ctx: &CommandContext<'_>) -> Result<(String, AuditDetail), AnalyticalError> {
        let missing = AnalyticalError::NotFound;
        Ok(match self {
            Self::Instrument(id) => {
                instrument_name(ctx.tx(), id)?;
                (id.to_string(), AuditDetail::id(id))
            }
            Self::Lot(id) => {
                lots::lot(ctx.tx(), id)?.ok_or(missing("lot"))?;
                (id.to_string(), AuditDetail::id(id))
            }
            Self::PrepBatch(id) => {
                lots::prep_batch(ctx.tx(), id)?.ok_or(missing("preparation batch"))?;
                (id.to_string(), AuditDetail::id(id))
            }
            Self::Run(id) => {
                runs::run(ctx.tx(), id)?.ok_or(missing("run"))?;
                (id.to_string(), AuditDetail::id(id))
            }
            Self::Position(id) => {
                runs::position(ctx.tx(), id)?.ok_or(missing("position"))?;
                (id.to_string(), AuditDetail::id(id))
            }
        })
    }
}

/// Input of [`open_deviation`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct OpenDeviationInput {
    /// What it applies to.
    pub scope: DeviationTarget,
    /// The one analyte it is limited to; none means every analyte.
    #[serde(default)]
    #[ts(optional)]
    pub analyte_id: Option<AnalyteId>,
    /// What happened, as entered.
    pub description: String,
    /// Why it is opened, as entered.
    pub reason: String,
}

/// Opens a deviation. Until it is closed, `dependency_status` reports
/// `deviation_open` for every dependent result.
pub fn open_deviation(
    ctx: &mut CommandContext<'_>,
    input: OpenDeviationInput,
) -> Result<DeviationView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let description = lis_core::required_text("description", &input.description, TEXT_MAX)?;
    let reason = reason(&input.reason)?;
    let (scope_id, target) = input.scope.check(ctx)?;
    if let Some(analyte_id) = input.analyte_id {
        let known = lis_catalog::list_analytes(ctx.tx())?;
        if !known.iter().any(|analyte| analyte.id == analyte_id) {
            return Err(AnalyticalError::NotFound("analyte"));
        }
    }
    let id = DeviationId::generate();
    let by = actor(ctx);
    repo::insert(
        ctx.tx(),
        &NewDeviation {
            id,
            scope: input.scope.scope(),
            scope_id: &scope_id,
            analyte_id: input.analyte_id,
            description: &description,
            reason: &reason,
            by: &by,
            at: ctx.now(),
        },
    )?;
    let mut event = clinical(actions::OPEN_DEVIATION)
        .record("deviation", id)
        .new_revision(1)
        .detail("scope", AuditDetail::code(input.scope.scope().as_str()))
        .detail("target", target)
        .reason(reason);
    if let Some(analyte_id) = input.analyte_id {
        event = event.detail("analyte", AuditDetail::id(analyte_id));
    }
    ctx.audit(event)?;
    deviation(ctx, id)
}

/// Input of [`close_deviation`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CloseDeviationInput {
    /// The deviation.
    pub deviation_id: DeviationId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The disposition, as entered.
    pub disposition: String,
    /// Why it is closed, as entered.
    pub reason: String,
}

/// Closes an open deviation with a disposition and reason.
pub fn close_deviation(
    ctx: &mut CommandContext<'_>,
    input: CloseDeviationInput,
) -> Result<DeviationView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let disposition = lis_core::required_text("disposition", &input.disposition, TEXT_MAX)?;
    let reason = reason(&input.reason)?;
    let current = deviation(ctx, input.deviation_id)?;
    ensure_revision(current.revision, input.expected_revision)?;
    if current.state != DeviationState::Open {
        return Err(AnalyticalError::WrongState("deviation"));
    }
    let by = actor(ctx);
    repo::close(
        ctx.tx(),
        current.deviation_id,
        current.revision + 1,
        &Closing {
            disposition: &disposition,
            reason: &reason,
            by: &by,
            at: ctx.now(),
        },
    )?;
    ctx.audit(
        clinical(actions::CLOSE_DEVIATION)
            .record("deviation", current.deviation_id)
            .old_revision(current.revision)
            .new_revision(current.revision + 1)
            .reason(reason),
    )?;
    deviation(ctx, current.deviation_id)
}

fn deviation(ctx: &CommandContext<'_>, id: DeviationId) -> Result<DeviationView, AnalyticalError> {
    repo::deviation(ctx.tx(), id)?.ok_or(AnalyticalError::NotFound("deviation"))
}
