//! Preservation incidents as the report page and the incidents queue show
//! them (NFR-07, REQ-OPS-004). A revision whose stored content fails its
//! digest is left out of the impact and delivery queues; until an incident
//! records it, the incidents queue lists it here.

use lis_core::{Capability, Principal, UtcTimestamp};
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::model::{
    PreservationIncidentId, PreservationIncidentKind, ReportId, ReportRevisionId,
    RevisionAvailability,
};
use crate::queries::candidate::user_view;
use crate::repo::candidates;
use crate::repo::quality::{self, IncidentSubject, StoredIncident};
use crate::repo::reports::{self, RevisionRow};
use crate::stored::{ContentIncident, content_incident};
use crate::views::{IncidentClosureView, PreservationIncidentView};

/// An incident with what `caller` may do with it: close it while open
/// (`manage_quality`).
pub(crate) fn incident_view(
    conn: &Connection,
    stored: StoredIncident,
    caller: &Principal,
) -> Result<PreservationIncidentView, ReportingError> {
    let revision = stored.revision();
    let StoredIncident {
        incident,
        report_id,
        closure,
    } = stored;
    let (revision_id, candidate_id, withdrawal_notice) = match incident.subject {
        IncidentSubject::Revision(id) => (Some(id), None, false),
        IncidentSubject::Candidate(id) => (None, Some(id), false),
        IncidentSubject::Withdrawal(_) => (None, None, true),
    };
    let closes = closure.is_none() && caller.require(Capability::ManageQuality).is_ok();
    Ok(PreservationIncidentView {
        incident_id: incident.id,
        revision,
        kind: incident.kind,
        record: incident.record,
        artifact_sha256: incident.artifact_sha256,
        report_id,
        revision_id,
        candidate_id,
        withdrawal_notice,
        detected_at: incident.detected_at,
        closure: closure
            .map(|c| -> Result<_, ReportingError> {
                Ok(IncidentClosureView {
                    resolution: c.resolution,
                    reason: c.reason,
                    closed_by: user_view(conn, c.closed_by)?,
                    closed_at: c.closed_at,
                })
            })
            .transpose()?,
        allowed_actions: if closes {
            vec!["close_preservation_incident".to_owned()]
        } else {
            Vec::new()
        },
    })
}

/// Every incident of a report, open or closed, oldest first.
pub(crate) fn report_incidents(
    conn: &Connection,
    report_id: ReportId,
    caller: &Principal,
) -> Result<Vec<PreservationIncidentView>, ReportingError> {
    quality::report_incidents(conn, report_id)?
        .into_iter()
        .map(|stored| incident_view(conn, stored, caller))
        .collect()
}

/// Stored content of `revision` that fails its digest and that no incident
/// covers yet ([`quality::known_incident`]).
fn unrecorded(
    conn: &Connection,
    revision: &RevisionRow,
) -> Result<Option<ContentIncident>, ReportingError> {
    let candidate = candidates::candidate(conn, revision.candidate_id)?
        .ok_or(ReportingError::Inconsistent("released candidate"))?;
    let Some(incident) = content_incident(conn, &candidate)? else {
        return Ok(None);
    };
    let known = quality::known_incident(conn, incident.subject, incident.record, &incident.sha256)?;
    Ok(known.is_none().then_some(incident))
}

/// The damaged stored content of `report_id`'s current revision that no
/// incident covers yet, for the engine to record before showing the report.
pub fn unrecorded_content_incident(
    conn: &Connection,
    report_id: ReportId,
) -> Result<Option<ContentIncident>, ReportingError> {
    match reports::latest_revision(conn, report_id)? {
        Some(revision) if revision.availability == RevisionAvailability::Current => {
            unrecorded(conn, &revision)
        }
        Some(_) | None => Ok(None),
    }
}

/// One item of the incidents queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentQueueItem {
    pub report_id: ReportId,
    pub report_number: String,
    /// The open incident, or the damaged revision no incident covers yet.
    pub subject: IncidentQueueSubject,
    pub kind: PreservationIncidentKind,
    /// When it was detected; for an unrecorded one, the revision's release.
    pub since: UtcTimestamp,
}

/// What an incidents queue item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentQueueSubject {
    /// A recorded, open incident.
    Incident(PreservationIncidentId),
    /// A current revision whose stored content fails its digest.
    UnrecordedRevision(ReportRevisionId),
}

/// Open incidents and the current revisions with damaged stored content no
/// incident covers yet, oldest first (NFR-07, REQ-OPS-004). Closed
/// incidents leave the queue; the report page still shows them.
pub fn preservation_incidents(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<IncidentQueueItem>, ReportingError> {
    let mut items = Vec::new();
    for stored in quality::open_incidents(conn, limit)? {
        items.push(IncidentQueueItem {
            report_number: report_number(conn, stored.report_id)?,
            report_id: stored.report_id,
            subject: IncidentQueueSubject::Incident(stored.incident.id),
            kind: stored.incident.kind,
            since: stored.incident.detected_at,
        });
    }
    for revision in reports::current_revisions(conn)? {
        if unrecorded(conn, &revision)?.is_some() {
            items.push(IncidentQueueItem {
                report_number: report_number(conn, revision.report_id)?,
                report_id: revision.report_id,
                subject: IncidentQueueSubject::UnrecordedRevision(revision.id),
                kind: PreservationIncidentKind::Corrupt,
                since: revision.released_at,
            });
        }
    }
    items.sort_by_key(|item| item.since);
    items.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    Ok(items)
}

fn report_number(conn: &Connection, report_id: ReportId) -> Result<String, ReportingError> {
    Ok(reports::report(conn, report_id)?
        .ok_or(ReportingError::Inconsistent("incident report"))?
        .report_number)
}
