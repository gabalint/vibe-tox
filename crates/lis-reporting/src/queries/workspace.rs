//! The release workspace of one specimen (brief §3.1 step 1): the content a
//! candidate prepared now would have, its dispositions, relationship and
//! gates. A hint only; preparation and release recheck everything.

use lis_core::{GateReason, UserId, UtcTimestamp};
use lis_specimens::SpecimenId;
use rusqlite::Connection;

use crate::error::{ReportingError, gate};
use crate::gates;
use crate::queries::candidate::{candidate_view, dispositions, user_view};
use crate::queries::report::revision_summary;
use crate::relationship::relationship;
use crate::repo::candidates;
use crate::repo::reports::{self, ReportRow, RevisionRow};
use crate::report_data::{Header, tox_report_data};
use crate::snapshot::{self, Built, ReportSnapshot};
use crate::views::{ReportPreview, ReportWorkspace, WorkspaceReport};

/// The workspace of `specimen_id` as `viewer` would prepare it at `now`.
/// PHI: an audited read (`release_reports`).
pub fn report_workspace(
    conn: &Connection,
    specimen_id: SpecimenId,
    viewer: UserId,
    now: UtcTimestamp,
) -> Result<ReportWorkspace, ReportingError> {
    let accession = lis_specimens::specimen_accession(conn, specimen_id)?;
    let accession = lis_specimens::get_accession(conn, accession)?;
    let specimen_type_code = accession
        .specimens
        .iter()
        .find(|s| s.specimen_id == specimen_id)
        .map(|s| s.specimen_type_code.clone())
        .ok_or(ReportingError::NotFound("specimen"))?;
    let report = reports::report_for_specimen(conn, specimen_id)?;
    let (current, withdrawn) = match &report {
        Some(report) => (
            reports::latest_revision(conn, report.id)?,
            reports::withdrawal(conn, report.id)?.is_some(),
        ),
        None => (None, false),
    };
    let built = snapshot::build(conn, specimen_id);
    let condition_review_source = match &built {
        Ok(built) => crate::condition_review::source(&built.snapshot.specimen)?,
        Err(_) => crate::ConditionReviewSource {
            source_sha256: String::new(),
            latest_decision_id: None,
            conditions: Vec::new(),
        },
    };
    let preview = if withdrawn {
        blocked_preview(vec![gate("report_withdrawn", "This report was withdrawn.")])
    } else {
        preview(conn, built, report.as_ref(), current.as_ref(), viewer, now)?
    };
    let open = open_candidate_view(conn, report.as_ref(), now)?;
    let can_review_conditions = !condition_review_source.conditions.is_empty()
        && preview
            .gates
            .iter()
            .all(|gate| gate.code == "nothing_changed");
    let allowed_actions = if preview.gates.is_empty() || can_review_conditions {
        vec!["prepare_report_candidate".to_owned()]
    } else {
        Vec::new()
    };
    Ok(ReportWorkspace {
        unavailable_candidate_id: open.unavailable,
        open_candidate_incident: open.incident,
        condition_review_source,
        specimen_id,
        report: report
            .map(|r| -> Result<_, ReportingError> {
                Ok(WorkspaceReport {
                    report_id: r.id,
                    report_number: r.report_number,
                    current_revision: current
                        .as_ref()
                        .map(|c| revision_summary(conn, c))
                        .transpose()?,
                    availability: current.as_ref().map(|c| c.availability),
                })
            })
            .transpose()?,
        subject: lis_identity::subject_summary(conn, accession.subject_id)?,
        accession_number: accession.accession_number,
        specimen_type_code,
        preview,
        open_candidate: open.view,
        allowed_actions,
    })
}

fn blocked_preview(gates: Vec<GateReason>) -> ReportPreview {
    ReportPreview {
        content: None,
        dispositions: Vec::new(),
        completeness: None,
        relationship: None,
        gates,
    }
}

fn preview(
    conn: &Connection,
    built: Result<Built, ReportingError>,
    report: Option<&ReportRow>,
    current: Option<&RevisionRow>,
    viewer: UserId,
    now: UtcTimestamp,
) -> Result<ReportPreview, ReportingError> {
    let mut built = match built {
        Ok(built) => built,
        Err(ReportingError::GateBlocked(reasons)) => return Ok(blocked_preview(reasons)),
        Err(error) => return Err(error),
    };
    crate::condition_review::preview(&mut built.snapshot.specimen);
    let mut gates = gates::content_gates(&built.snapshot, &built.manifest);
    gates.extend(gates::evaluate(conn, &built.snapshot)?);
    let current_content = current
        .map(|revision| ReportSnapshot::of_revision(conn, revision))
        .transpose()?;
    let relationship = match relationship(current_content.as_ref(), &built.snapshot) {
        Ok(relationship) => Some(relationship),
        Err(ReportingError::NothingChanged) => {
            gates.push(gate(
                "nothing_changed",
                "Nothing changed since the current revision.",
            ));
            None
        }
        Err(error) => return Err(error),
    };
    let content = match relationship {
        Some(relationship) if gates.is_empty() => {
            let header = Header {
                report_number: report_number(report, &built),
                revision_no: current.map_or(1, |r| r.revision_no + 1),
                relationship,
                supersedes: current.map(|r| (r.revision_no, r.released_at)),
                amendment_reason: None,
                prepared_at: now,
                released_by: user_view(conn, viewer)?.display_name,
                change_summary: crate::relationship::change_summary(
                    current_content.as_ref(),
                    &built.snapshot,
                ),
            };
            Some(tox_report_data(&built.snapshot, &header)?)
        }
        Some(_) | None => None,
    };
    Ok(ReportPreview {
        content,
        dispositions: dispositions(&built.snapshot, Some(&built)),
        completeness: Some(crate::dispositions::completeness(
            crate::report_data::dispositions(&built.snapshot),
        )),
        relationship,
        gates,
    })
}

fn report_number(report: Option<&ReportRow>, built: &Built) -> String {
    report.map_or_else(
        || built.snapshot.specimen.report_number(),
        |r| r.report_number.clone(),
    )
}

#[derive(Default)]
struct OpenCandidate {
    view: Option<crate::CandidateView>,
    unavailable: Option<crate::CandidateId>,
    incident: Option<crate::ContentIncident>,
}

/// A damaged unissued draft cannot hide a fresh clinical workspace or be displayed as trusted.
fn open_candidate_view(
    conn: &Connection,
    report: Option<&ReportRow>,
    now: UtcTimestamp,
) -> Result<OpenCandidate, ReportingError> {
    let candidate = report
        .map(|report| candidates::open_candidate(conn, report.id))
        .transpose()?
        .flatten();
    let Some(candidate) = candidate else {
        return Ok(OpenCandidate::default());
    };
    match candidate_view(conn, &candidate, now) {
        Ok(view) => Ok(OpenCandidate {
            view: Some(view),
            ..OpenCandidate::default()
        }),
        Err(ReportingError::ContentMismatch(incident)) => Ok(OpenCandidate {
            unavailable: Some(candidate.id),
            incident: Some(incident),
            view: None,
        }),
        Err(error) => Err(error),
    }
}
