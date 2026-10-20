//! SQL for impact assessments and preservation incidents and their closures.

use lis_audit::AuditDetail;
use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    CandidateId, ImpactAssessmentId, ImpactDecision, IncidentResolution, PreservationIncidentId,
    PreservationIncidentKind, ReportId, ReportRevisionId, StoredRecord,
};

/// One impact assessment with the keys it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssessmentRow {
    pub(crate) id: ImpactAssessmentId,
    pub(crate) revision_id: ReportRevisionId,
    pub(crate) decision: ImpactDecision,
    pub(crate) reason: String,
    pub(crate) assessed_by: UserId,
    pub(crate) assessed_at: UtcTimestamp,
    pub(crate) keys: Vec<String>,
}

/// A revision's assessments, oldest first.
pub(crate) fn assessments(
    conn: &Connection,
    revision_id: ReportRevisionId,
) -> rusqlite::Result<Vec<AssessmentRow>> {
    let mut rows: Vec<AssessmentRow> = conn
        .prepare_cached(
            "SELECT id, revision_id, decision, reason, assessed_by, assessed_at \
             FROM rpt_impact_assessments WHERE revision_id = ?1 ORDER BY assessed_at, id",
        )?
        .query_map([revision_id], |row| {
            Ok(AssessmentRow {
                id: row.get(0)?,
                revision_id: row.get(1)?,
                decision: row.get(2)?,
                reason: row.get(3)?,
                assessed_by: row.get(4)?,
                assessed_at: row.get(5)?,
                keys: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut keys = conn.prepare_cached(
        "SELECT reason_key FROM rpt_impact_assessment_keys WHERE assessment_id = ?1 \
         ORDER BY reason_key",
    )?;
    for row in &mut rows {
        row.keys = keys
            .query_map([row.id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(rows)
}

pub(crate) fn insert_assessment(conn: &Connection, a: &AssessmentRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_impact_assessments (id, revision_id, decision, reason, assessed_by, \
         assessed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        a.id,
        a.revision_id,
        a.decision,
        a.reason,
        a.assessed_by,
        a.assessed_at
    ])?;
    let mut key = conn.prepare_cached(
        "INSERT INTO rpt_impact_assessment_keys (assessment_id, reason_key) VALUES (?1, ?2)",
    )?;
    for reason_key in &a.keys {
        key.execute(params![a.id, reason_key])?;
    }
    Ok(())
}

/// What an incident is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentSubject {
    /// A released revision's PDF.
    Revision(ReportRevisionId),
    /// A withdrawal notice.
    Withdrawal(ReportId),
    /// A ready candidate's PDF.
    Candidate(CandidateId),
}

impl IncidentSubject {
    /// The subject as an audit detail, such as `("revision_id", id)`.
    #[must_use]
    pub fn audit_detail(self) -> (&'static str, AuditDetail) {
        match self {
            Self::Revision(id) => ("revision_id", AuditDetail::id(id)),
            Self::Withdrawal(id) => ("report_id", AuditDetail::id(id)),
            Self::Candidate(id) => ("candidate_id", AuditDetail::id(id)),
        }
    }
}

/// One preservation incident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IncidentRow {
    pub(crate) id: PreservationIncidentId,
    pub(crate) artifact_sha256: String,
    pub(crate) subject: IncidentSubject,
    pub(crate) kind: PreservationIncidentKind,
    pub(crate) record: StoredRecord,
    pub(crate) detected_by: String,
    pub(crate) detected_at: UtcTimestamp,
}

/// A closure of an incident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClosureRow {
    pub(crate) incident_id: PreservationIncidentId,
    pub(crate) resolution: IncidentResolution,
    pub(crate) reason: String,
    pub(crate) closed_by: UserId,
    pub(crate) closed_at: UtcTimestamp,
}

/// An incident as stored, with its report and its closure, if closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredIncident {
    pub(crate) incident: IncidentRow,
    pub(crate) report_id: ReportId,
    pub(crate) closure: Option<ClosureRow>,
}

impl StoredIncident {
    /// 1 while open, 2 once closed.
    pub(crate) fn revision(&self) -> i64 {
        if self.closure.is_some() { 2 } else { 1 }
    }
}

fn subject_columns(
    subject: IncidentSubject,
) -> (
    Option<ReportRevisionId>,
    Option<ReportId>,
    Option<CandidateId>,
) {
    match subject {
        IncidentSubject::Revision(id) => (Some(id), None, None),
        IncidentSubject::Withdrawal(id) => (None, Some(id), None),
        IncidentSubject::Candidate(id) => (None, None, Some(id)),
    }
}

pub(crate) fn insert_incident(conn: &Connection, i: &IncidentRow) -> rusqlite::Result<()> {
    let (revision, withdrawal, candidate) = subject_columns(i.subject);
    conn.prepare_cached(
        "INSERT INTO rpt_preservation_incidents (id, artifact_sha256, revision_id, \
         withdrawal_report_id, candidate_id, kind, stored_record, detected_by, detected_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        i.id,
        i.artifact_sha256,
        revision,
        withdrawal,
        candidate,
        i.kind,
        i.record,
        i.detected_by,
        i.detected_at
    ])?;
    Ok(())
}

pub(crate) fn insert_closure(conn: &Connection, c: &ClosureRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_preservation_incident_closures (incident_id, resolution, reason, \
         closed_by, closed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        c.incident_id,
        c.resolution,
        c.reason,
        c.closed_by,
        c.closed_at
    ])?;
    Ok(())
}

/// The incident that already covers a detection of `sha256` failing for
/// `record` of `subject`: the latest such incident, when it is open or its
/// loss was accepted. A detection it covers adds no incident.
pub(crate) fn known_incident(
    conn: &Connection,
    subject: IncidentSubject,
    record: StoredRecord,
    sha256: &str,
) -> rusqlite::Result<Option<PreservationIncidentId>> {
    let (revision, withdrawal, candidate) = subject_columns(subject);
    let latest: Option<(PreservationIncidentId, Option<IncidentResolution>)> = conn
        .prepare_cached(
            "SELECT i.id, x.resolution FROM rpt_preservation_incidents i \
             LEFT JOIN rpt_preservation_incident_closures x ON x.incident_id = i.id \
             WHERE i.revision_id IS ?1 AND i.withdrawal_report_id IS ?2 \
               AND i.candidate_id IS ?3 AND i.stored_record = ?4 AND i.artifact_sha256 = ?5 \
             ORDER BY i.detected_at DESC, i.id DESC LIMIT 1",
        )?
        .query_row(
            params![revision, withdrawal, candidate, record, sha256],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(latest.and_then(|(id, resolution)| match resolution {
        None | Some(IncidentResolution::AcceptedAsLost) => Some(id),
        Some(
            IncidentResolution::ArtifactRestoredFromBackup | IncidentResolution::RevisionReissued,
        ) => None,
    }))
}

const INCIDENT_SELECT: &str = "SELECT i.id, i.artifact_sha256, i.revision_id, \
     i.withdrawal_report_id, i.candidate_id, i.kind, i.stored_record, i.detected_by, \
     i.detected_at, coalesce(r.report_id, i.withdrawal_report_id, c.report_id), \
     x.resolution, x.reason, x.closed_by, x.closed_at \
     FROM rpt_preservation_incidents i \
     LEFT JOIN rpt_revisions r ON r.id = i.revision_id \
     LEFT JOIN rpt_candidates c ON c.id = i.candidate_id \
     LEFT JOIN rpt_preservation_incident_closures x ON x.incident_id = i.id";

/// One incident.
pub(crate) fn incident(
    conn: &Connection,
    id: PreservationIncidentId,
) -> rusqlite::Result<Option<StoredIncident>> {
    conn.prepare_cached(&format!("{INCIDENT_SELECT} WHERE i.id = ?1"))?
        .query_row([id], stored_incident)
        .optional()
}

/// Open incidents, oldest first.
pub(crate) fn open_incidents(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<StoredIncident>> {
    conn.prepare_cached(&format!(
        "{INCIDENT_SELECT} WHERE x.incident_id IS NULL ORDER BY i.detected_at, i.id LIMIT ?1"
    ))?
    .query_map([limit], stored_incident)?
    .collect()
}

/// Every incident of a report, open or closed, oldest first.
pub(crate) fn report_incidents(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Vec<StoredIncident>> {
    conn.prepare_cached(&format!(
        "{INCIDENT_SELECT} WHERE coalesce(r.report_id, i.withdrawal_report_id, c.report_id) = ?1 \
         ORDER BY i.detected_at, i.id"
    ))?
    .query_map([report_id], stored_incident)?
    .collect()
}

fn stored_incident(row: &Row<'_>) -> rusqlite::Result<StoredIncident> {
    let subject = match (row.get(2)?, row.get(3)?, row.get(4)?) {
        (Some(revision), _, _) => IncidentSubject::Revision(revision),
        (None, Some(report), _) => IncidentSubject::Withdrawal(report),
        (None, None, Some(candidate)) => IncidentSubject::Candidate(candidate),
        (None, None, None) => {
            return Err(rusqlite::Error::InvalidColumnType(
                2,
                "incident subject".to_owned(),
                rusqlite::types::Type::Null,
            ));
        }
    };
    let id = row.get(0)?;
    let closure = match row.get::<_, Option<IncidentResolution>>(10)? {
        Some(resolution) => Some(ClosureRow {
            incident_id: id,
            resolution,
            reason: row.get(11)?,
            closed_by: row.get(12)?,
            closed_at: row.get(13)?,
        }),
        None => None,
    };
    Ok(StoredIncident {
        incident: IncidentRow {
            id,
            artifact_sha256: row.get(1)?,
            subject,
            kind: row.get(5)?,
            record: row.get(6)?,
            detected_by: row.get(7)?,
            detected_at: row.get(8)?,
        },
        report_id: row.get(9)?,
        closure,
    })
}
