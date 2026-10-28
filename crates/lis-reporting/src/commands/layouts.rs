//! Report layout choice (REQ-RPT-007): an administrator drafts a ready-made
//! layout; a quality manager approves it, which makes it the layout of every
//! candidate prepared afterwards, or rejects it with a reason. The author
//! approves their own draft only where the laboratory's separation-of-duties
//! policy allows it and they confirm it (ADR 0015). Issued reports never change.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::Duty;
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::{authorize_self, user_id};
use crate::error::{ReportingError, invalid, required};
use crate::layouts::{self, ReportLayout};
use crate::model::{LayoutVersionId, LayoutVersionState};
use crate::queries::layouts::report_layouts;
use crate::repo::layouts::{self as repo, LayoutVersionRow};
use crate::views::ReportLayoutsView;

const RECORD_TYPE: &str = "report_layout_version";

/// Input of `draft_report_layout`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DraftReportLayoutInput {
    /// The ready-made layout to propose.
    pub layout: ReportLayout,
}

/// Input of `approve_report_layout`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ApproveReportLayoutInput {
    /// The draft.
    pub version_id: LayoutVersionId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The approver confirms they drafted this layout themselves and approve it
    /// with no second person, as the laboratory's separation-of-duties policy
    /// allows (ADR 0015). Needed only in that case.
    pub self_approval: bool,
}

/// Input of `reject_report_layout`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RejectReportLayoutInput {
    /// The draft.
    pub version_id: LayoutVersionId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why the layout is rejected.
    pub reason: String,
}

/// Drafts a layout choice. It affects no report until approved.
pub fn draft_report_layout(
    ctx: &mut CommandContext<'_>,
    input: DraftReportLayoutInput,
) -> Result<ReportLayoutsView, ReportingError> {
    ctx.require(Capability::ManageConfiguration)?;
    let drafted_by = user_id(ctx)?;
    if repo::draft(ctx.tx())?.is_some() {
        return Err(ReportingError::Duplicate("layout draft"));
    }
    if layouts::in_force(ctx.tx())? == input.layout {
        return Err(invalid(
            "layout",
            "already_in_force",
            "This layout is already the active layout.",
        ));
    }
    let version = LayoutVersionRow {
        id: LayoutVersionId::generate(),
        version_no: repo::next_version_no(ctx.tx())?,
        layout: input.layout,
        state: LayoutVersionState::Draft,
        drafted_by,
        drafted_at: ctx.now(),
        decided_by: None,
        decided_at: None,
        rejection_reason: None,
        superseded_at: None,
        revision: 1,
    };
    repo::insert(ctx.tx(), &version)?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, actions::DRAFT_LAYOUT, Outcome::Succeeded)
            .record(RECORD_TYPE, version.id)
            .new_revision(1)
            .detail("version_no", AuditDetail::integer(version.version_no))
            .detail("layout", AuditDetail::code(version.layout.as_str())),
    )?;
    report_layouts(ctx.tx(), ctx.principal())
}

/// Approves the draft: it becomes the active layout and the previous one is
/// superseded. Candidates already prepared keep the layout they recorded.
pub fn approve_report_layout(
    ctx: &mut CommandContext<'_>,
    input: ApproveReportLayoutInput,
) -> Result<ReportLayoutsView, ReportingError> {
    ctx.require(Capability::ManageQuality)?;
    let approver = user_id(ctx)?;
    let draft = open_draft(ctx, input.version_id, input.expected_revision)?;
    let by_author = draft.drafted_by == approver;
    if by_author {
        authorize_self(
            ctx.tx(),
            Duty::ConfigurationApproval,
            input.self_approval,
            "second_person_required",
        )?;
    }
    let previous = repo::active(ctx.tx())?;
    if let Some(previous) = &previous {
        repo::supersede(ctx.tx(), previous.id, ctx.now())?;
    }
    decide(ctx, &draft, approver, None)?;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        actions::APPROVE_LAYOUT,
        Outcome::Succeeded,
    )
    .record(RECORD_TYPE, draft.id)
    .old_revision(draft.revision)
    .new_revision(draft.revision + 1)
    .detail("version_no", AuditDetail::integer(draft.version_no))
    .detail("layout", AuditDetail::code(draft.layout.as_str()))
    .detail("drafted_by", AuditDetail::id(draft.drafted_by));
    if let Some(previous) = previous {
        event = event.detail("superseded_version_id", AuditDetail::id(previous.id));
    }
    if by_author {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    report_layouts(ctx.tx(), ctx.principal())
}

/// Rejects the draft with a reason; the active layout stays in force.
pub fn reject_report_layout(
    ctx: &mut CommandContext<'_>,
    input: RejectReportLayoutInput,
) -> Result<ReportLayoutsView, ReportingError> {
    ctx.require(Capability::ManageQuality)?;
    let by = user_id(ctx)?;
    let reason = required("reason", &input.reason, 1000)?;
    let draft = open_draft(ctx, input.version_id, input.expected_revision)?;
    decide(ctx, &draft, by, Some(&reason))?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, actions::REJECT_LAYOUT, Outcome::Succeeded)
            .record(RECORD_TYPE, draft.id)
            .old_revision(draft.revision)
            .new_revision(draft.revision + 1)
            .detail("version_no", AuditDetail::integer(draft.version_no))
            .detail("layout", AuditDetail::code(draft.layout.as_str()))
            .reason(reason),
    )?;
    report_layouts(ctx.tx(), ctx.principal())
}

/// The draft at the revision shown.
fn open_draft(
    ctx: &CommandContext<'_>,
    id: LayoutVersionId,
    expected_revision: i64,
) -> Result<LayoutVersionRow, ReportingError> {
    let version = repo::get(ctx.tx(), id)?.ok_or(ReportingError::NotFound("layout version"))?;
    if version.revision != expected_revision {
        return Err(ReportingError::RevisionConflict);
    }
    if version.state != LayoutVersionState::Draft {
        return Err(ReportingError::WrongState("layout version"));
    }
    Ok(version)
}

fn decide(
    ctx: &CommandContext<'_>,
    draft: &LayoutVersionRow,
    by: lis_core::UserId,
    rejection_reason: Option<&str>,
) -> Result<(), ReportingError> {
    if repo::decide(
        ctx.tx(),
        draft.id,
        draft.revision,
        by,
        ctx.now(),
        rejection_reason,
    )? {
        Ok(())
    } else {
        Err(ReportingError::RevisionConflict)
    }
}
