//! The candidate a releaser reviews: its exact printed strings, with gates
//! and staleness recomputed on read (brief §3.1 step 3). A hint only: release
//! rechecks everything in its own transaction.

use lis_core::{GateReason, UserId, UtcTimestamp};
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::gates;
use crate::model::{CandidateId, CandidateState, LineDisposition};
use crate::policy::CANDIDATE_MAX_AGE;
use crate::repo::candidates::{self, CandidateRow};
use crate::repo::reports;
use crate::snapshot::{self, Built, ReportSnapshot, sha256_hex};
use crate::stored::{StoredContent, stored_content};
use crate::views::{CandidateView, DispositionView, ReportUserView};

/// One candidate. Report content: an audited read (`release_reports`).
pub fn get_candidate(
    conn: &Connection,
    candidate_id: CandidateId,
    now: UtcTimestamp,
) -> Result<CandidateView, ReportingError> {
    let candidate = candidates::candidate(conn, candidate_id)?
        .ok_or(ReportingError::NotFound("report candidate"))?;
    candidate_view(conn, &candidate, now)
}

/// The digest of a ready candidate's stored PDF, which release reads back
/// and verifies before its transaction (brief §3.1 step 4), after its
/// stored content and data pass their digests.
pub fn candidate_artifact(
    conn: &Connection,
    candidate_id: CandidateId,
) -> Result<String, ReportingError> {
    let candidate = candidates::candidate(conn, candidate_id)?
        .ok_or(ReportingError::NotFound("report candidate"))?;
    stored_content(conn, &candidate)?;
    match candidate.render {
        Some(rendered) if candidate.state == CandidateState::Ready => Ok(rendered.artifact_sha256),
        Some(_) | None => Err(ReportingError::CandidateNotReady),
    }
}

pub(crate) fn candidate_view(
    conn: &Connection,
    candidate: &CandidateRow,
    now: UtcTimestamp,
) -> Result<CandidateView, ReportingError> {
    let report = reports::report(conn, candidate.report_id)?
        .ok_or(ReportingError::Inconsistent("candidate report"))?;
    let StoredContent {
        snapshot: frozen,
        data: content,
    } = stored_content(conn, candidate)?;
    let mut gates = gates::evaluate(conn, &frozen)?;
    gates.extend(crate::condition_review::release_gate(&frozen.specimen));
    // A gate that prevents rebuilding the content (no laboratory profile)
    // makes the candidate stale; any other failure is an error.
    let fresh = match snapshot::build(conn, report.specimen_id) {
        Ok(built) => Ok(built),
        Err(ReportingError::GateBlocked(reasons)) => Err(reasons),
        Err(error) => return Err(error),
    };
    let (stale, stale_reasons) = staleness(candidate, &frozen, &fresh)?;
    let expires_at = expiry(candidate.prepared_at)?;
    let supersedes_revision_no = match candidate.supersedes_revision_id {
        Some(id) => reports::revision(conn, id)?.map(|r| r.revision_no),
        None => None,
    };
    let releasable =
        candidate.state == CandidateState::Ready && !stale && gates.is_empty() && now <= expires_at;
    Ok(CandidateView {
        candidate_id: candidate.id,
        revision: candidate.revision,
        state: candidate.state,
        report_id: report.id,
        report_number: report.report_number,
        proposed_revision_no: candidate.proposed_revision_no,
        relationship: candidate.relationship,
        completeness: candidate.completeness,
        supersedes_revision_no,
        amendment_reason: candidate.amendment_reason.clone(),
        content_sha256: candidate.content_sha256.clone(),
        artifact_sha256: candidate.render.as_ref().map(|r| r.artifact_sha256.clone()),
        prepared_at: candidate.prepared_at,
        expires_at,
        signatory: user_view(conn, candidate.signatory_user_id)?,
        content,
        dispositions: dispositions(&frozen, fresh.as_ref().ok()),
        gates,
        stale,
        stale_reasons,
        allowed_actions: if releasable {
            vec!["release_report".to_owned()]
        } else {
            Vec::new()
        },
    })
}

/// When a candidate prepared at `prepared_at` may no longer be released (L5).
pub(crate) fn expiry(prepared_at: UtcTimestamp) -> Result<UtcTimestamp, ReportingError> {
    prepared_at
        .checked_add(CANDIDATE_MAX_AGE)
        .ok_or(ReportingError::Inconsistent("candidate time"))
}

/// Whether the content rebuilt now differs, and what holds the lines that changed.
fn staleness(
    candidate: &CandidateRow,
    frozen: &ReportSnapshot,
    fresh: &Result<Built, Vec<GateReason>>,
) -> Result<(bool, Vec<GateReason>), ReportingError> {
    let built = match fresh {
        Ok(built) => built,
        Err(reasons) => return Ok((true, reasons.clone())),
    };
    let mut compared = built.snapshot.clone();
    compared.specimen.condition_report_review = frozen.specimen.condition_report_review.clone();
    if sha256_hex(&compared.to_canonical_json()?) == candidate.content_sha256 {
        return Ok((false, Vec::new()));
    }
    let mut reasons = Vec::new();
    for line in &built.snapshot.lines {
        let before = frozen
            .lines
            .iter()
            .find(|f| f.component_id == line.component_id)
            .map(|f| f.disposition);
        if before != Some(line.disposition) {
            reasons.extend(holds_of(built, line.component_id).iter().cloned());
        }
    }
    Ok((true, reasons))
}

fn holds_of(built: &Built, component_id: lis_orders::OrderComponentId) -> &[GateReason] {
    built
        .holds
        .iter()
        .find(|(id, _)| *id == component_id)
        .map_or(&[], |(_, holds)| holds.as_slice())
}

/// The snapshot's lines with what holds each one now, when known.
pub(crate) fn dispositions(
    snapshot: &ReportSnapshot,
    fresh: Option<&Built>,
) -> Vec<DispositionView> {
    let components = snapshot.lines.iter().map(|line| DispositionView {
        component_id: Some(line.component_id),
        name: line.orderable_name.clone(),
        disposition: line.disposition,
        gates: match (line.disposition, fresh) {
            (LineDisposition::Reported, _) | (_, None) => Vec::new(),
            (_, Some(built)) => holds_of(built, line.component_id).to_vec(),
        },
    });
    let triggers = snapshot.triggers.iter().map(|line| DispositionView {
        component_id: None,
        name: line.orderable_name.clone(),
        disposition: line.disposition,
        gates: Vec::new(),
    });
    components.chain(triggers).collect()
}

/// A user's display name, for record views.
pub(crate) fn user_view(
    conn: &Connection,
    user_id: UserId,
) -> Result<ReportUserView, ReportingError> {
    let user = lis_auth::session_user(conn, user_id)?;
    Ok(ReportUserView {
        user_id,
        display_name: user.display_name,
    })
}
