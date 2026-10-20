//! Report-level, terminal withdrawal with a minimal notice (brief §4.2,
//! REQ-RPT-002, REQ-RPT-013, L12). Never gated by holds: it is the safety
//! action. Fresh authentication and preflight commit before the engine renders
//! anything; the stored notice is then durably staged before withdrawal.

use std::fmt;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::Reauthentication;
use lis_auth::{Attempt, ReauthPurpose};
use lis_core::{Capability, UserId, UtcTimestamp};
use lis_report::ReportLaboratory;
use lis_report::notice_v1::{ReportNoticeData, ReportNoticeKind, ReportNoticeSection};
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::{current_user, user_id};
use crate::commands::routing::withdrawal_plan;
use crate::error::{ReportingError, gate, required};
use crate::model::{CandidateState, ReportId};
use crate::queries::report::get_report;
use crate::repo::candidates;
use crate::repo::reports::{self, WithdrawalRow};
use crate::views::ReportDetail;

/// Input of `withdraw_report`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct WithdrawReportInput {
    /// The report.
    pub report_id: ReportId,
    /// The current revision number shown.
    #[ts(type = "number")]
    pub expected_revision_no: i64,
    /// Why, as entered; kept on the record, never on the notice.
    pub reason: String,
    /// Fresh confirmation for this action (REQ-IDN-004).
    pub password: Reauthentication,
}

/// The stored withdrawal notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeArtifact {
    /// Digest of the stored notice PDF.
    pub artifact_sha256: String,
    /// The `report-notice` template version.
    pub template_version: i64,
    /// The revision number the notice names.
    pub revision_no: i64,
}

/// The notice to render before withdrawing.
#[derive(Clone, PartialEq, Eq)]
pub struct NoticeDraft {
    /// The `report-notice` v1 JSON.
    pub data_json: String,
    /// The current revision it names.
    pub revision_no: i64,
    /// The committed preflight time used for PDF metadata too.
    pub issued_at: UtcTimestamp,
}

impl fmt::Debug for NoticeDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NoticeDraft")
            .field("revision_no", &self.revision_no)
            .field("issued_at", &self.issued_at)
            .finish_non_exhaustive()
    }
}

/// A withdrawal that passed fresh authentication and every preflight check.
/// The password is deliberately absent from this value.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedWithdrawal {
    pub(crate) report_id: ReportId,
    pub(crate) expected_revision_no: i64,
    pub(crate) reason: String,
    /// The notice that may now be rendered.
    pub notice: NoticeDraft,
}

impl PreparedWithdrawal {
    /// The report that passed preflight.
    #[must_use]
    pub fn report_id(&self) -> ReportId {
        self.report_id
    }
}

impl fmt::Debug for PreparedWithdrawal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedWithdrawal")
            .field("report_id", &self.report_id)
            .field("expected_revision_no", &self.expected_revision_no)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

/// The withdrawal notice of `report_id` as `issued_by` would issue it at `now`:
/// the laboratory, report number, current revision and accession, and no
/// demographics, results or reason.
fn withdrawal_notice(
    conn: &Connection,
    report_id: ReportId,
    issued_by: UserId,
    now: UtcTimestamp,
) -> Result<NoticeDraft, ReportingError> {
    let report = reports::report(conn, report_id)?.ok_or(ReportingError::NotFound("report"))?;
    if reports::withdrawal(conn, report_id)?.is_some() {
        return Err(ReportingError::ReportWithdrawn);
    }
    let revision =
        reports::latest_revision(conn, report_id)?.ok_or(ReportingError::WrongState("report"))?;
    let profile = lis_identity::current_laboratory_profile(conn)?.ok_or_else(|| {
        ReportingError::GateBlocked(vec![gate(
            "laboratory_profile_missing",
            "Set the laboratory profile before issuing notices.",
        )])
    })?;
    let accession = lis_specimens::specimen_accession(conn, report.specimen_id)?;
    let accession = lis_specimens::get_accession(conn, accession)?;
    let data = ReportNoticeData {
        laboratory: ReportLaboratory {
            name: profile.name,
            address_lines: profile.address_lines,
            clia_id: profile.clia_id,
            director: profile.director,
            phone: profile.phone,
        },
        notice: ReportNoticeSection {
            report_number: report.report_number,
            revision: revision.revision_no.to_string(),
            accession: accession.accession_number,
            kind: ReportNoticeKind::Withdrawal,
            issued_at: crate::display::lab_time(now, &profile.time_zone)?,
            issued_by: lis_auth::session_user(conn, issued_by)?.display_name,
        },
    };
    Ok(NoticeDraft {
        data_json: lis_report::notice_v1::to_json(&data)?,
        revision_no: revision.revision_no,
        issued_at: now,
    })
}

/// Commits fresh authentication and validates the withdrawal before any
/// PHI-bearing notice is rendered or stored.
pub fn preflight_withdrawal(
    ctx: &mut CommandContext<'_>,
    input: WithdrawReportInput,
) -> Result<Attempt<PreparedWithdrawal>, ReportingError> {
    let user = current_user(ctx, Capability::ReleaseReports)?;
    if let Attempt::Rejected(rejection) =
        lis_auth::reauthenticate(ctx, &input.password, ReauthPurpose::ReportWithdrawal)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    let reason = required("reason", &input.reason, 1000)?;
    reports::report(ctx.tx(), input.report_id)?.ok_or(ReportingError::NotFound("report"))?;
    if reports::withdrawal(ctx.tx(), input.report_id)?.is_some() {
        return Err(ReportingError::ReportWithdrawn);
    }
    let current = reports::latest_revision(ctx.tx(), input.report_id)?
        .ok_or(ReportingError::WrongState("report"))?;
    if current.revision_no != input.expected_revision_no {
        return Err(ReportingError::RevisionConflict);
    }
    let notice = withdrawal_notice(ctx.tx(), input.report_id, user.user_id, ctx.now())?;
    Ok(Attempt::Accepted(PreparedWithdrawal {
        report_id: input.report_id,
        expected_revision_no: input.expected_revision_no,
        reason,
        notice,
    }))
}

/// Records the content-addressed notice immediately after storage. This is a
/// committed reconciliation record if the final withdrawal later fails.
pub fn stage_withdrawal_notice(
    ctx: &mut CommandContext<'_>,
    prepared: &PreparedWithdrawal,
    notice: &NoticeArtifact,
) -> Result<(), ReportingError> {
    // Preflight already checked current grants. This staging transaction must
    // still reconcile the just-stored artifact if the grant was revoked in
    // between; the final withdrawal rechecks current grants again.
    ctx.require(Capability::ReleaseReports)?;
    if notice.revision_no != prepared.expected_revision_no {
        return Err(ReportingError::RevisionConflict);
    }
    let row = reports::StagedWithdrawalNoticeRow {
        artifact_sha256: notice.artifact_sha256.clone(),
        report_id: prepared.report_id,
        revision_no: notice.revision_no,
        template_version: notice.template_version,
        staged_by: user_id(ctx)?,
        staged_at: ctx.now(),
    };
    reports::insert_staged_withdrawal_notice(ctx.tx(), &row)?;
    let recorded = reports::staged_withdrawal_notice(ctx.tx(), &notice.artifact_sha256)?
        .ok_or(ReportingError::Inconsistent("staged withdrawal notice"))?;
    if recorded.report_id != row.report_id
        || recorded.revision_no != row.revision_no
        || recorded.template_version != row.template_version
    {
        return Err(ReportingError::Inconsistent("staged withdrawal notice"));
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::STAGE_WITHDRAWAL_NOTICE,
            Outcome::Succeeded,
        )
        .record("report", prepared.report_id)
        .detail(
            "notice_sha256",
            AuditDetail::sha256(&notice.artifact_sha256)?,
        )
        .detail("revision_no", AuditDetail::integer(notice.revision_no))
        .detail(
            "notice_template_version",
            AuditDetail::integer(notice.template_version),
        ),
    )?;
    Ok(())
}

/// Withdraws a preflighted report: every revision becomes
/// `withdrawn`, every destination that ever had an obligation gets a notice
/// obligation, open report obligations close, and an open candidate is
/// superseded. The stored notice must already have a durable staging record.
pub fn withdraw_report(
    ctx: &mut CommandContext<'_>,
    prepared: PreparedWithdrawal,
    notice: &NoticeArtifact,
) -> Result<ReportDetail, ReportingError> {
    let user = current_user(ctx, Capability::ReleaseReports)?;
    reports::report(ctx.tx(), prepared.report_id)?.ok_or(ReportingError::NotFound("report"))?;
    if reports::withdrawal(ctx.tx(), prepared.report_id)?.is_some() {
        return Err(ReportingError::ReportWithdrawn);
    }
    let current = reports::latest_revision(ctx.tx(), prepared.report_id)?
        .ok_or(ReportingError::WrongState("report"))?;
    if current.revision_no != prepared.expected_revision_no
        || current.revision_no != notice.revision_no
    {
        return Err(ReportingError::RevisionConflict);
    }
    let staged = reports::staged_withdrawal_notice(ctx.tx(), &notice.artifact_sha256)?
        .ok_or(ReportingError::Inconsistent("staged withdrawal notice"))?;
    if staged.report_id != prepared.report_id
        || staged.revision_no != notice.revision_no
        || staged.template_version != notice.template_version
    {
        return Err(ReportingError::Inconsistent("staged withdrawal notice"));
    }
    let plan = withdrawal_plan(ctx.tx(), prepared.report_id)?;
    reports::insert_withdrawal(
        ctx.tx(),
        &WithdrawalRow {
            report_id: prepared.report_id,
            revision_id: current.id,
            reason: prepared.reason.clone(),
            notice_artifact_sha256: notice.artifact_sha256.clone(),
            notice_template_version: notice.template_version,
            withdrawn_by: user.user_id,
            withdrawn_at: ctx.now(),
        },
    )?;
    plan.apply(ctx, current.id)?;
    if let Some(open) = candidates::open_candidate(ctx.tx(), prepared.report_id)?
        && !candidates::set_state(ctx.tx(), open.id, open.revision, CandidateState::Superseded)?
    {
        return Err(ReportingError::RevisionConflict);
    }
    ctx.audit(
        AuditEvent::new(Stream::Clinical, actions::WITHDRAW, Outcome::Succeeded)
            .record("report", prepared.report_id)
            .detail("revision_id", AuditDetail::id(current.id))
            .detail("revision_no", AuditDetail::integer(current.revision_no))
            .detail(
                "notice_sha256",
                AuditDetail::sha256(&notice.artifact_sha256)?,
            )
            .detail(
                "notice_template_version",
                AuditDetail::integer(notice.template_version),
            )
            .detail(
                "obligation_count",
                AuditDetail::count(plan.obligation_count()),
            )
            .reason(prepared.reason),
    )?;
    get_report(ctx.tx(), prepared.report_id, ctx.principal())
}
