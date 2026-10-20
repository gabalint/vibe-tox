//! Quality decision on impact after release (brief §4.3, REQ-RPT-002, L16).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::user_id;
use crate::error::{ReportingError, invalid, required};
use crate::impact;
use crate::model::{ImpactAssessmentId, ImpactDecision, ReportRevisionId, RevisionAvailability};
use crate::queries::report::impact_view;
use crate::repo::quality::{self, AssessmentRow};
use crate::repo::reports;
use crate::views::ImpactView;

/// Input of `record_impact_assessment`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordImpactAssessmentInput {
    /// The current revision.
    pub revision_id: ReportRevisionId,
    /// The open reason keys this assessment covers.
    pub keys: Vec<String>,
    /// No impact, or a correction is required.
    pub decision: ImpactDecision,
    /// Why, as entered.
    pub reason: String,
}

/// Records an assessment covering open impact keys of a current revision.
/// A key that appears later reopens the impact.
pub fn record_impact_assessment(
    ctx: &mut CommandContext<'_>,
    input: RecordImpactAssessmentInput,
) -> Result<ImpactView, ReportingError> {
    ctx.require(Capability::ManageQuality)?;
    let assessed_by = user_id(ctx)?;
    let reason = required("reason", &input.reason, 1000)?;
    let revision = reports::revision(ctx.tx(), input.revision_id)?
        .ok_or(ReportingError::NotFound("report revision"))?;
    if revision.availability != RevisionAvailability::Current {
        return Err(ReportingError::WrongState("report revision"));
    }
    let open = impact::impact(ctx.tx(), &revision)?.open;
    if input.keys.is_empty() {
        return Err(invalid(
            "keys",
            "required",
            "Select at least one open reason.",
        ));
    }
    let mut keys = Vec::new();
    for key in input.keys {
        if !open.iter().any(|reason| reason.key == key) {
            return Err(invalid(
                "keys",
                "not_open",
                "A selected reason is not open.",
            ));
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let assessment = AssessmentRow {
        id: ImpactAssessmentId::generate(),
        revision_id: revision.id,
        decision: input.decision,
        reason: reason.clone(),
        assessed_by,
        assessed_at: ctx.now(),
        keys,
    };
    quality::insert_assessment(ctx.tx(), &assessment)?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, actions::ASSESS_IMPACT, Outcome::Succeeded)
            .record("impact_assessment", assessment.id)
            .detail("revision_id", AuditDetail::id(revision.id))
            .detail("decision", AuditDetail::code(assessment.decision.as_str()))
            .detail("key_count", AuditDetail::count(assessment.keys.len()))
            .reason(reason),
    )?;
    impact_view(ctx.tx(), &revision)
}
