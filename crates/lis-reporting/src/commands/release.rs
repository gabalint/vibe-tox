//! Release: the final transaction of brief §3.4 and its writes (§3.5).
//! Every gate is read here, through its owner's function, never from an
//! earlier read (NFR-01). The PDF was read and verified by the engine before
//! this transaction; only its digest is compared here. So is the
//! laboratory's separation-of-duties policy, when the releaser reviewed a
//! reported result (row 13, ADR 0015).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::Reauthentication;
use lis_auth::{Attempt, Duty, ReauthPurpose, SessionUser};
use lis_core::{Capability, UserId};
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::{authorize_self, current_user};
use crate::commands::routing::{ObligationPlan, release_plan};
use crate::error::{ReportingError, gate, invalid};
use crate::gates;
use crate::model::{
    CandidateId, CandidateState, LineDisposition, ReportCompleteness, ReportRevisionId,
    RevisionAvailability,
};
use crate::policy;
use crate::queries::candidate::{expiry, user_view};
use crate::repo::candidates::{self, CandidateRow, Rendered};
use crate::repo::reports::{self, RevisionRow};
use crate::snapshot::{self, Built, sha256_hex};
use crate::stored::stored_content;
use crate::views::RevisionSummary;

/// Input of `release_report`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ReleaseReportInput {
    /// The candidate reviewed.
    pub candidate_id: CandidateId,
    /// The candidate revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The content digest shown.
    pub content_sha256: String,
    /// The releaser acknowledges a partial report (L3).
    pub acknowledge_partial: bool,
    /// Fresh confirmation for this release (REQ-IDN-004).
    pub password: Reauthentication,
    /// The releaser confirms they reviewed results on this report themselves and
    /// release it with no second person, as the laboratory's separation-of-duties
    /// policy allows (ADR 0015). Needed only in that case.
    pub self_approval: bool,
}

/// Releases a candidate as the next revision (REQ-RPT-001, REQ-RPT-002,
/// REQ-RPT-005, REQ-RPT-013, REQ-RES-005, REQ-IDN-004, NFR-01).
/// `verified_artifact_sha256` is the digest of the bytes the engine read back
/// intact. A wrong password returns the committed rejection and changes
/// nothing else.
pub fn release_candidate(
    ctx: &mut CommandContext<'_>,
    input: ReleaseReportInput,
    verified_artifact_sha256: &str,
) -> Result<Attempt<RevisionSummary>, ReportingError> {
    let user = current_user(ctx, Capability::ReleaseReports)?;
    if let Attempt::Rejected(rejection) =
        lis_auth::reauthenticate(ctx, &input.password, ReauthPurpose::ReportRelease)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    let candidate = releasable_candidate(ctx, &input, user.user_id)?;
    let current = current_revision(ctx, &candidate)?;
    let rendered = candidate
        .render
        .as_ref()
        .ok_or(ReportingError::CandidateNotReady)?;
    if rendered.artifact_sha256 != verified_artifact_sha256 {
        return Err(ReportingError::RevisionConflict);
    }
    let frozen = stored_content(ctx.tx(), &candidate)?.snapshot;
    let mut rebuilt = snapshot::build(ctx.tx(), frozen.specimen.specimen_id)?;
    rebuilt.snapshot.specimen.condition_report_review =
        frozen.specimen.condition_report_review.clone();
    if sha256_hex(&rebuilt.snapshot.to_canonical_json()?) != candidate.content_sha256 {
        return Err(ReportingError::CandidateStale(gates::evaluate(
            ctx.tx(),
            &frozen,
        )?));
    }
    check_review_attribution(ctx, &candidate, &rebuilt)?;
    let mut blocking = gates::evaluate(ctx.tx(), &frozen)?;
    blocking.extend(crate::condition_review::release_gate(&frozen.specimen));
    if !blocking.is_empty() {
        return Err(ReportingError::GateBlocked(blocking));
    }
    let self_release = check_two_person(ctx, &rebuilt, user.user_id, input.self_approval)?;
    if policy::PARTIAL_RELEASE_NEEDS_ACKNOWLEDGEMENT
        && candidate.completeness == ReportCompleteness::Partial
        && !input.acknowledge_partial
    {
        return Err(invalid(
            "acknowledgePartial",
            "required",
            "Acknowledge that this report is partial.",
        ));
    }
    let plan = release_plan(ctx.tx(), candidate.report_id, frozen.order.order_id)?;
    write_release(
        ctx,
        &candidate,
        current.as_ref(),
        &plan,
        &user,
        self_release,
    )
    .map(Attempt::Accepted)
}

/// Row 4: ready, the revision and content shown, the caller's own, not expired.
fn releasable_candidate(
    ctx: &CommandContext<'_>,
    input: &ReleaseReportInput,
    user_id: UserId,
) -> Result<CandidateRow, ReportingError> {
    let candidate = candidates::candidate(ctx.tx(), input.candidate_id)?
        .ok_or(ReportingError::NotFound("report candidate"))?;
    if candidate.state != CandidateState::Ready {
        return Err(ReportingError::CandidateNotReady);
    }
    if candidate.revision != input.expected_revision
        || candidate.content_sha256 != input.content_sha256
    {
        return Err(ReportingError::RevisionConflict);
    }
    if candidate.signatory_user_id != user_id {
        return Err(ReportingError::NotSignatory);
    }
    if ctx.now() > expiry(candidate.prepared_at)? {
        return Err(ReportingError::CandidateExpired);
    }
    Ok(candidate)
}

/// Row 5: not withdrawn, and the candidate follows the current revision.
fn current_revision(
    ctx: &CommandContext<'_>,
    candidate: &CandidateRow,
) -> Result<Option<RevisionRow>, ReportingError> {
    if reports::withdrawal(ctx.tx(), candidate.report_id)?.is_some() {
        return Err(ReportingError::ReportWithdrawn);
    }
    let current = reports::latest_revision(ctx.tx(), candidate.report_id)?;
    let follows = current.as_ref().map(|r| r.id) == candidate.supersedes_revision_id
        && current.as_ref().map_or(1, |r| r.revision_no + 1) == candidate.proposed_revision_no;
    if !follows {
        return Err(ReportingError::RevisionConflict);
    }
    Ok(current)
}

/// Each reported line's frozen review attribution is the current one. The
/// content digest leaves reviews out, so without this a re-review after
/// preparation would let row 13 judge reviews the candidate does not name.
fn check_review_attribution(
    ctx: &CommandContext<'_>,
    candidate: &CandidateRow,
    rebuilt: &Built,
) -> Result<(), ReportingError> {
    let replaced = candidates::lines(ctx.tx(), candidate.id)?
        .iter()
        .filter(|line| line.disposition == LineDisposition::Reported)
        .any(|line| {
            let current = rebuilt
                .lines
                .iter()
                .find(|now| {
                    line.order_component_id == Some(now.component_id)
                        && line.analyte_id == Some(now.analyte_id)
                })
                .and_then(|now| now.review)
                .map(|review| (review.review_id, review.reviewed_by));
            current != line.review
        });
    if replaced {
        return Err(ReportingError::CandidateStale(vec![gate(
            "review_stale:review_replaced",
            "A reported result was reviewed again after the candidate was prepared.",
        )]));
    }
    Ok(())
}

/// Row 13 (L4, REQ-RES-005, ADR 0015): whether the releaser reviewed a
/// reported line, by its current review read in this transaction. If so, the
/// separation-of-duties policy in force must allow it and the releaser must
/// confirm it: `self_release` while it needs a second person, a validation
/// error on `selfApproval` while unconfirmed.
fn check_two_person(
    ctx: &CommandContext<'_>,
    rebuilt: &Built,
    releaser: UserId,
    confirmed: bool,
) -> Result<bool, ReportingError> {
    let reported: Vec<_> = rebuilt
        .snapshot
        .reported()
        .map(|(line, analyte)| (line.component_id, analyte.analyte_id))
        .collect();
    let reviewers: Vec<UserId> = rebuilt
        .lines
        .iter()
        .filter(|line| reported.contains(&(line.component_id, line.analyte_id)))
        .filter_map(|line| line.review.map(|r| r.reviewed_by))
        .collect();
    self_release(ctx.tx(), releaser, &reviewers, confirmed)
}

/// Whether `releaser` reviewed a reported line (`reviewers`), and if so that
/// the policy in force in `conn`'s transaction allows it and they confirmed it.
fn self_release(
    conn: &Connection,
    releaser: UserId,
    reviewers: &[UserId],
    confirmed: bool,
) -> Result<bool, ReportingError> {
    if !policy::is_self_release(releaser, reviewers) {
        return Ok(false);
    }
    authorize_self(conn, Duty::ReportRelease, confirmed, "self_release")?;
    Ok(true)
}

/// §3.5: the candidate becomes `released`; the `report.release` event is
/// the release manifest, marked when the releaser reviewed a reported result
/// (`self_release`); the revision row follows it (trigger); then the
/// obligations, committed with the release (no outbox in M3).
fn write_release(
    ctx: &CommandContext<'_>,
    candidate: &CandidateRow,
    current: Option<&RevisionRow>,
    plan: &ObligationPlan,
    user: &SessionUser,
    self_release: bool,
) -> Result<RevisionSummary, ReportingError> {
    if !candidates::set_state(
        ctx.tx(),
        candidate.id,
        candidate.revision,
        CandidateState::Released,
    )? {
        return Err(ReportingError::RevisionConflict);
    }
    let rendered = candidate
        .render
        .as_ref()
        .ok_or(ReportingError::CandidateNotReady)?;
    let revision = RevisionRow {
        id: ReportRevisionId::generate(),
        report_id: candidate.report_id,
        revision_no: candidate.proposed_revision_no,
        candidate_id: candidate.id,
        relationship: candidate.relationship,
        completeness: candidate.completeness,
        supersedes_revision_id: current.map(|r| r.id),
        amendment_reason: candidate.amendment_reason.clone(),
        content_sha256: candidate.content_sha256.clone(),
        artifact_sha256: rendered.artifact_sha256.clone(),
        released_by: user.user_id,
        released_at: ctx.now(),
        availability: RevisionAvailability::Current,
    };
    let mut event = release_event(candidate, rendered, &revision, plan.obligation_count())?;
    if self_release {
        // ADR 0015: a release by the reviewer is listed among self-approvals.
        event = event.self_approval();
    }
    ctx.audit(event)?;
    reports::insert_revision(ctx.tx(), &revision)?;
    plan.apply(ctx, revision.id)?;
    Ok(RevisionSummary {
        revision_id: revision.id,
        revision_no: revision.revision_no,
        relationship: revision.relationship,
        completeness: revision.completeness,
        availability: RevisionAvailability::Current,
        released_at: revision.released_at,
        released_by: user_view(ctx.tx(), user.user_id)?,
        amendment_reason: revision.amendment_reason,
        artifact_sha256: revision.artifact_sha256,
    })
}

/// The release manifest: the `report.release` event the revision trigger
/// requires (brief §3.5 step 1).
fn release_event(
    candidate: &CandidateRow,
    rendered: &Rendered,
    revision: &RevisionRow,
    obligation_count: usize,
) -> Result<AuditEvent, ReportingError> {
    let mut event = AuditEvent::new(Stream::Clinical, actions::RELEASE, Outcome::Succeeded)
        .record("report_revision", revision.id)
        .new_revision(revision.revision_no)
        .detail("report_id", AuditDetail::id(candidate.report_id))
        .detail("candidate_id", AuditDetail::id(candidate.id))
        .detail(
            "content_sha256",
            AuditDetail::sha256(&candidate.content_sha256)?,
        )
        .detail("data_sha256", AuditDetail::sha256(&candidate.data_sha256)?)
        .detail(
            "artifact_sha256",
            AuditDetail::sha256(&rendered.artifact_sha256)?,
        )
        .detail(
            "template_sha256",
            AuditDetail::sha256(&rendered.template_sha256)?,
        )
        .detail(
            "template",
            AuditDetail::machine_token(&candidate.template_id)?,
        )
        .detail(
            "template_version",
            AuditDetail::integer(candidate.template_version),
        )
        .detail(
            "renderer",
            AuditDetail::machine_token(&rendered.renderer_version.replace(' ', "-"))?,
        )
        .detail(
            "relationship",
            AuditDetail::code(candidate.relationship.as_str()),
        )
        .detail(
            "completeness",
            AuditDetail::code(candidate.completeness.as_str()),
        )
        .detail("obligation_count", AuditDetail::count(obligation_count));
    if let Some(supersedes) = revision.supersedes_revision_id {
        event = event.detail("supersedes_revision_id", AuditDetail::id(supersedes));
    }
    if let Some(reason) = &candidate.amendment_reason {
        event = event.reason(reason.clone());
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use lis_core::ErrorCode;
    use rusqlite::params;

    use super::*;

    /// The `auth_` schema alone, which holds the separation-of-duties policy.
    fn schema() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        for migration in [lis_audit::MIGRATIONS, lis_auth::MIGRATIONS].concat() {
            conn.execute_batch(migration.sql).unwrap();
        }
        conn
    }

    /// Appends policy version `version`, allowing only self-release when `allowed`.
    fn set_policy(conn: &Connection, version: i64, allowed: bool) {
        conn.execute(
            "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
             self_release_reports, self_verify_destinations, reason, set_by, set_at) \
             VALUES (?1, 1, ?2, 1, 'Synthetic policy', 'u9', '2026-10-14T09:00:00.000000000Z')",
            params![version, allowed],
        )
        .unwrap();
    }

    fn gate_codes(error: &ReportingError) -> Vec<String> {
        error.gate_reasons().into_iter().map(|g| g.code).collect()
    }

    // REQ-RES-005, ADR 0015: under the default policy a reviewer may not
    // release, confirmed or not.
    #[test]
    fn a_reviewer_may_not_release_under_the_default_policy() {
        let conn = schema();
        let releaser = UserId::generate();
        for confirmed in [false, true] {
            let refused = self_release(&conn, releaser, &[releaser], confirmed).unwrap_err();
            assert_eq!(refused.code(), ErrorCode::GateBlocked);
            assert_eq!(gate_codes(&refused), ["self_release"]);
            assert!(
                refused.gate_reasons()[0]
                    .message
                    .contains("separation-of-duties")
            );
        }
    }

    // ADR 0015: an allowed self-release still needs the releaser's
    // confirmation, asked for on the `selfApproval` field.
    #[test]
    fn an_allowed_self_release_needs_its_confirmation() {
        let conn = schema();
        set_policy(&conn, 1, true);
        let releaser = UserId::generate();
        let refused = self_release(&conn, releaser, &[releaser], false).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::Validation);
        let fields = refused.field_errors();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field, "selfApproval");
        assert_eq!(fields[0].code, "self_approval_unconfirmed");
        assert!(self_release(&conn, releaser, &[releaser], true).unwrap());
    }

    // REQ-RES-005: a release by someone who reviewed none of the reported
    // lines is not a self-release, under any policy, confirmed or not.
    #[test]
    fn a_release_by_another_person_is_not_a_self_release() {
        let conn = schema();
        let (releaser, reviewer) = (UserId::generate(), UserId::generate());
        for confirmed in [false, true] {
            assert!(!self_release(&conn, releaser, &[reviewer], confirmed).unwrap());
            assert!(!self_release(&conn, releaser, &[], confirmed).unwrap());
        }
    }

    // NFR-01, ADR 0015: the policy is read at the check, so a version
    // committed after preparation governs the release.
    #[test]
    fn the_policy_in_force_at_release_governs() {
        let conn = schema();
        let releaser = UserId::generate();
        assert!(self_release(&conn, releaser, &[releaser], true).is_err());
        set_policy(&conn, 1, true);
        assert!(self_release(&conn, releaser, &[releaser], true).unwrap());
        set_policy(&conn, 2, false);
        let refused = self_release(&conn, releaser, &[releaser], true).unwrap_err();
        assert_eq!(gate_codes(&refused), ["self_release"]);
    }
}
