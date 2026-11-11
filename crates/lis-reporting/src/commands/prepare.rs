//! Candidate preparation (brief §3.1). T1 stores the candidate in
//! `rendering`; the engine renders and stores the PDF outside any
//! transaction; T2 records the artifact, or the failure.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp};
use lis_report::TemplateRef;
use lis_specimens::SpecimenId;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::current_user;
use crate::error::{ReportingError, invalid, optional};
use crate::gates;
use crate::model::{
    CandidateId, CandidateState, LineDisposition, LineKind, ReportId, ReportRelationship,
};
use crate::relationship::relationship;
use crate::repo::candidates::{self, CandidateRow, LineRow};
use crate::repo::reports::{self, ReportRow, RevisionRow};
use crate::report_data::{Header, tox_report_data};
use crate::snapshot::{self, Built, ReportSnapshot, sha256_hex};
mod rendering;
pub use rendering::{RenderFailure, RenderedArtifact, record_render_failure, record_rendered};

/// Input of `prepare_report_candidate`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PrepareReportCandidateInput {
    /// The specimen to report.
    pub specimen_id: SpecimenId,
    /// Required for, and only for, a correction (REQ-RPT-002).
    #[serde(default)]
    #[ts(optional)]
    pub amendment_reason: Option<String>,
    /// Required when receipt conditions exist; freezes explicit disclosure decisions.
    #[serde(default)]
    #[ts(optional)]
    pub condition_review: Option<crate::ConditionReportReview>,
}

/// A stored candidate the engine must render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCandidate {
    /// The candidate.
    pub candidate_id: CandidateId,
    /// The exact template input.
    pub data_json: String,
    /// The template of the layout in force at preparation, recorded on the candidate.
    pub template: TemplateRef,
    /// The printed issue time.
    pub prepared_at: UtcTimestamp,
}

/// T1: rechecks authority and the report-level gates in this transaction,
/// builds the snapshot, supersedes the open candidate and stores the new
/// one in `rendering` with its lines (REQ-RPT-001, REQ-RPT-005, NFR-01).
pub fn prepare_candidate(
    ctx: &mut CommandContext<'_>,
    input: PrepareReportCandidateInput,
) -> Result<PreparedCandidate, ReportingError> {
    let user = current_user(ctx, Capability::ReleaseReports)?;
    let reason = optional("amendmentReason", input.amendment_reason.as_deref(), 1000)?;
    let existing = reports::report_for_specimen(ctx.tx(), input.specimen_id)?;
    if let Some(report) = &existing
        && reports::withdrawal(ctx.tx(), report.id)?.is_some()
    {
        return Err(ReportingError::ReportWithdrawn);
    }
    let mut built = snapshot::build(ctx.tx(), input.specimen_id)?;
    crate::condition_review::apply(&mut built.snapshot.specimen, input.condition_review)?;
    let mut blocking = gates::content_gates(&built.snapshot, &built.manifest);
    blocking.extend(gates::evaluate(ctx.tx(), &built.snapshot)?);
    if !blocking.is_empty() {
        return Err(ReportingError::GateBlocked(blocking));
    }
    let current = match &existing {
        Some(report) => reports::latest_revision(ctx.tx(), report.id)?,
        None => None,
    };
    let current_content = current
        .as_ref()
        .map(|revision| ReportSnapshot::of_revision(ctx.tx(), revision))
        .transpose()?;
    let relationship = relationship(current_content.as_ref(), &built.snapshot)?;
    let reason = match (relationship, reason) {
        (ReportRelationship::Correction, None) => {
            return Err(invalid(
                "amendmentReason",
                "required",
                "A correction needs a reason.",
            ));
        }
        (ReportRelationship::Correction, Some(reason)) => Some(reason),
        (ReportRelationship::Initial | ReportRelationship::Supplement, Some(_)) => {
            return Err(invalid(
                "amendmentReason",
                "not_a_correction",
                "Only a correction takes an amendment reason.",
            ));
        }
        (ReportRelationship::Initial | ReportRelationship::Supplement, None) => None,
    };
    let report = match existing {
        Some(report) => report,
        None => create_report(ctx, &built)?,
    };
    let header = Header {
        report_number: report.report_number.clone(),
        revision_no: current.as_ref().map_or(1, |r| r.revision_no + 1),
        relationship,
        supersedes: current.as_ref().map(|r| (r.revision_no, r.released_at)),
        amendment_reason: reason,
        prepared_at: ctx.now(),
        released_by: user.display_name,
        change_summary: crate::relationship::change_summary(
            current_content.as_ref(),
            &built.snapshot,
        ),
    };
    store_candidate(ctx, &report, &built, current.as_ref(), &header)
}

/// L2: accession number, `-`, specimen type code.
fn create_report(ctx: &CommandContext<'_>, built: &Built) -> Result<ReportRow, ReportingError> {
    let specimen = &built.snapshot.specimen;
    let report = ReportRow {
        id: ReportId::generate(),
        specimen_id: specimen.specimen_id,
        report_number: specimen.report_number(),
    };
    let by = crate::authority::user_id(ctx)?;
    reports::insert_report(ctx.tx(), &report, &by.to_string(), ctx.now())?;
    Ok(report)
}

fn store_candidate(
    ctx: &CommandContext<'_>,
    report: &ReportRow,
    built: &Built,
    current: Option<&RevisionRow>,
    header: &Header,
) -> Result<PreparedCandidate, ReportingError> {
    let data = tox_report_data(&built.snapshot, header)?;
    let content_json = built.snapshot.to_canonical_json()?;
    let data_json = lis_report::tox_v2::to_json(&data)?;
    let template = crate::layouts::in_force(ctx.tx())?.template();
    let superseded = candidates::open_candidate(ctx.tx(), report.id)?;
    if let Some(open) = &superseded
        && !candidates::set_state(ctx.tx(), open.id, open.revision, CandidateState::Superseded)?
    {
        return Err(ReportingError::RevisionConflict);
    }
    let candidate = CandidateRow {
        id: CandidateId::generate(),
        report_id: report.id,
        proposed_revision_no: header.revision_no,
        relationship: header.relationship,
        supersedes_revision_id: current.map(|r| r.id),
        amendment_reason: header.amendment_reason.clone(),
        completeness: crate::dispositions::completeness(crate::report_data::dispositions(
            &built.snapshot,
        )),
        signatory_user_id: crate::authority::user_id(ctx)?,
        prepared_at: header.prepared_at,
        content_sha256: sha256_hex(&content_json),
        content_json,
        data_sha256: sha256_hex(&data_json),
        data_json,
        template_id: template.id.clone(),
        template_version: i64::from(template.version),
        state: CandidateState::Rendering,
        render: None,
        failure_code: None,
        revision: 1,
    };
    candidates::insert(ctx.tx(), &candidate)?;
    insert_lines(ctx, candidate.id, built)?;
    record_preparation(ctx, report, built, &candidate, superseded.as_ref())?;
    Ok(PreparedCandidate {
        candidate_id: candidate.id,
        data_json: candidate.data_json,
        template,
        prepared_at: candidate.prepared_at,
    })
}

/// One line per expected analyte, then one per open or unmet reflex trigger.
fn record_preparation(
    ctx: &CommandContext<'_>,
    report: &ReportRow,
    built: &Built,
    candidate: &CandidateRow,
    superseded: Option<&CandidateRow>,
) -> Result<(), ReportingError> {
    let mut event = AuditEvent::new(
        Stream::Clinical,
        actions::PREPARE_CANDIDATE,
        Outcome::Succeeded,
    )
    .record("report_candidate", candidate.id)
    .new_revision(1)
    .detail("report_id", AuditDetail::id(report.id))
    .detail("specimen_id", AuditDetail::id(report.specimen_id))
    .detail(
        "proposed_revision_no",
        AuditDetail::integer(candidate.proposed_revision_no),
    )
    .detail(
        "relationship",
        AuditDetail::code(candidate.relationship.as_str()),
    )
    .detail(
        "completeness",
        AuditDetail::code(candidate.completeness.as_str()),
    )
    .detail(
        "content_sha256",
        AuditDetail::sha256(&candidate.content_sha256)?,
    )
    .detail("data_sha256", AuditDetail::sha256(&candidate.data_sha256)?);
    if let Some(review) = &built.snapshot.specimen.condition_report_review {
        event = event
            .detail(
                "condition_source_sha256",
                AuditDetail::sha256(&review.source_sha256)?,
            )
            .detail(
                "condition_review_count",
                AuditDetail::integer(
                    i64::try_from(review.decisions.len())
                        .map_err(|_| ReportingError::Inconsistent("receipt condition count"))?,
                ),
            );
    }
    if let Some(open) = superseded {
        event = event.detail("superseded_candidate_id", AuditDetail::id(open.id));
    }
    if let Some(reason) = &candidate.amendment_reason {
        event = event.reason(reason.clone());
    }
    ctx.audit(event)?;
    Ok(())
}

fn insert_lines(
    ctx: &CommandContext<'_>,
    candidate_id: CandidateId,
    built: &Built,
) -> Result<(), ReportingError> {
    let mut line_no = 0;
    for component in &built.snapshot.lines {
        for analyte in &component.analytes {
            let facts = built
                .lines
                .iter()
                .find(|f| {
                    f.component_id == component.component_id && f.analyte_id == analyte.analyte_id
                })
                .ok_or(ReportingError::Inconsistent("candidate line"))?;
            let reported = component.disposition == LineDisposition::Reported;
            line_no += 1;
            candidates::insert_line(
                ctx.tx(),
                candidate_id,
                line_no,
                &LineRow {
                    kind: LineKind::Result,
                    order_component_id: Some(component.component_id),
                    analyte_id: Some(analyte.analyte_id),
                    parent_item_id: None,
                    orderable_id: None,
                    disposition: component.disposition,
                    result: facts.result,
                    review: facts
                        .review
                        .filter(|_| reported)
                        .map(|r| (r.review_id, r.reviewed_by)),
                },
            )?;
        }
    }
    for trigger in &built.snapshot.triggers {
        line_no += 1;
        candidates::insert_line(
            ctx.tx(),
            candidate_id,
            line_no,
            &LineRow {
                kind: LineKind::Trigger,
                order_component_id: None,
                analyte_id: None,
                parent_item_id: Some(trigger.parent_item_id),
                orderable_id: Some(trigger.orderable_id),
                disposition: trigger.disposition,
                result: None,
                review: None,
            },
        )?;
    }
    Ok(())
}
