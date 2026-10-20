//! An attributable factual resolution for previously unknown performing sites (RPT-05).

use lis_audit::AuditDetail;
use lis_core::Capability;
use lis_identity::LaboratoryProfileId;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{TEXT_MAX, clinical, reason, require_user};
use crate::actions;
use crate::error::{AnalyticalError, ensure_revision};
use crate::model::RunId;
use crate::repo::runs as repo;
use crate::views::RunDetail;

/// Recover site provenance from retained evidence, without replacing a known site.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RecordRunPerformingSiteInput {
    /// The run whose performing site is unknown.
    pub run_id: RunId,
    /// Run revision reviewed by the quality manager.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Exact historical laboratory profile supported by the evidence.
    pub laboratory_profile_id: LaboratoryProfileId,
    /// Retained external evidence identifying the performing site.
    pub source_reference: String,
    /// Why this evidence establishes the run's performing site.
    pub reason: String,
}

/// Records a single immutable resolution. Repeating the request or changing an
/// already known site is refused; original runs and issued reports stay unchanged.
pub fn record_run_performing_site(
    ctx: &mut CommandContext<'_>,
    input: RecordRunPerformingSiteInput,
) -> Result<RunDetail, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let by = require_user(ctx)?;
    let run = repo::run(ctx.tx(), input.run_id)?.ok_or(AnalyticalError::NotFound("run"))?;
    ensure_revision(run.revision, input.expected_revision)?;
    if crate::queries::run_laboratory_profile(ctx.tx(), input.run_id)?.is_some() {
        return Err(AnalyticalError::WrongState("performing site already known"));
    }
    lis_identity::get_laboratory_profile_by_id(ctx.tx(), input.laboratory_profile_id)?;
    let source = lis_core::required_text("sourceReference", &input.source_reference, TEXT_MAX)?;
    let reason = reason(&input.reason)?;
    repo::insert_site_resolution(ctx.tx(), &input, &source, &reason, &by, ctx.now())?;
    ctx.audit(
        clinical(actions::RECORD_RUN_PERFORMING_SITE)
            .record("run", input.run_id)
            .detail("run_revision", AuditDetail::integer(run.revision))
            .detail(
                "laboratory_profile",
                AuditDetail::id(input.laboratory_profile_id),
            )
            .reason(reason),
    )?;
    crate::queries::get_run(ctx.tx(), input.run_id, ctx.now())
}
