//! Preparation T2: attach the exact artifact or record a recoverable rendering failure.
use crate::model::{CandidateId, CandidateState};
use crate::queries::candidate::candidate_view;
use crate::repo::candidates::{self, CandidateRow, Rendered};
use crate::views::CandidateView;
use crate::{actions, error::ReportingError};
use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::{CommandContext, StoredArtifact};

/// The stored PDF and the provenance rendering reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedArtifact {
    /// Digest and size of the stored bytes.
    pub artifact: StoredArtifact,
    /// Digest of the template source.
    pub template_sha256: String,
    /// The renderer version.
    pub renderer_version: String,
    /// Bundled fonts the PDF uses.
    pub fonts: Vec<String>,
}

/// T2: the candidate becomes `ready` with its artifact (NFR-07). Refused
/// with a conflict when it is no longer `rendering`, for example because a
/// newer candidate superseded it.
pub fn record_rendered(
    ctx: &mut CommandContext<'_>,
    candidate_id: CandidateId,
    rendered: &RenderedArtifact,
) -> Result<CandidateView, ReportingError> {
    ctx.require(Capability::ReleaseReports)?;
    let candidate = rendering_candidate(ctx, candidate_id)?;
    let size = i64::try_from(rendered.artifact.size)
        .map_err(|_| ReportingError::Inconsistent("artifact size"))?;
    let stored = Rendered {
        artifact_sha256: rendered.artifact.digest.to_string(),
        artifact_size: size,
        template_sha256: rendered.template_sha256.clone(),
        renderer_version: rendered.renderer_version.clone(),
        fonts_json: serde_json::to_string(&rendered.fonts)?,
        rendered_at: ctx.now(),
    };
    if !candidates::set_ready(ctx.tx(), candidate_id, candidate.revision, &stored)? {
        return Err(ReportingError::RevisionConflict);
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::RENDER_CANDIDATE,
            Outcome::Succeeded,
        )
        .record("report_candidate", candidate_id)
        .old_revision(candidate.revision)
        .new_revision(candidate.revision + 1)
        .detail(
            "artifact_sha256",
            AuditDetail::sha256(&stored.artifact_sha256)?,
        )
        .detail(
            "template_sha256",
            AuditDetail::sha256(&stored.template_sha256)?,
        )
        .detail("artifact_size", AuditDetail::integer(size)),
    )?;
    let candidate = candidates::candidate(ctx.tx(), candidate_id)?
        .ok_or(ReportingError::NotFound("report candidate"))?;
    candidate_view(ctx.tx(), &candidate, ctx.now())
}

/// Why rendering produced no artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFailure {
    /// The template refused the data or the PDF export failed.
    Render,
    /// The bytes could not be stored.
    Store,
}

impl RenderFailure {
    fn code(self) -> &'static str {
        match self {
            Self::Render => "render_failed",
            Self::Store => "artifact_not_stored",
        }
    }
}

/// T2 after a failure: the candidate becomes `failed` with a code.
pub fn record_render_failure(
    ctx: &mut CommandContext<'_>,
    candidate_id: CandidateId,
    failure: RenderFailure,
) -> Result<(), ReportingError> {
    ctx.require(Capability::ReleaseReports)?;
    let candidate = rendering_candidate(ctx, candidate_id)?;
    if !candidates::set_failed(ctx.tx(), candidate_id, candidate.revision, failure.code())? {
        return Err(ReportingError::RevisionConflict);
    }
    ctx.audit(
        AuditEvent::new(Stream::Clinical, actions::RENDER_FAILED, Outcome::Failed)
            .record("report_candidate", candidate_id)
            .old_revision(candidate.revision)
            .new_revision(candidate.revision + 1)
            .detail("failure_code", AuditDetail::code(failure.code())),
    )?;
    Ok(())
}

fn rendering_candidate(
    ctx: &CommandContext<'_>,
    candidate_id: CandidateId,
) -> Result<CandidateRow, ReportingError> {
    let candidate = candidates::candidate(ctx.tx(), candidate_id)?
        .ok_or(ReportingError::NotFound("report candidate"))?;
    if candidate.state != CandidateState::Rendering {
        return Err(ReportingError::WrongState("report candidate"));
    }
    Ok(candidate)
}
