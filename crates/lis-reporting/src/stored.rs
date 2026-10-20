//! A candidate's stored content and printed data, each verified against its
//! digest before it is parsed (NFR-07). Every read of stored report JSON
//! goes through [`stored_content`]: display, release, relationship and
//! impact. A mismatch is a preservation incident, never silently shown;
//! the engine records it in its own transaction.

use lis_report::tox_v2::ToxReportData;
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::model::{CandidateState, StoredRecord};
use crate::repo::candidates::CandidateRow;
use crate::repo::quality::IncidentSubject;
use crate::repo::reports;
use crate::snapshot::{ReportSnapshot, sha256_hex};

/// A stored record whose bytes no longer match their digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIncident {
    /// The released revision, or the candidate not yet released.
    pub subject: IncidentSubject,
    /// Which record failed.
    pub record: StoredRecord,
    /// The digest the record should have.
    pub sha256: String,
}

/// A candidate's verified content and printed data.
pub(crate) struct StoredContent {
    pub(crate) snapshot: ReportSnapshot,
    pub(crate) data: ToxReportData,
}

/// Verifies both digests of `candidate`, then parses its JSON.
pub(crate) fn stored_content(
    conn: &Connection,
    candidate: &CandidateRow,
) -> Result<StoredContent, ReportingError> {
    if let Some(incident) = content_incident(conn, candidate)? {
        return Err(ReportingError::ContentMismatch(incident));
    }
    Ok(StoredContent {
        snapshot: serde_json::from_str(&candidate.content_json)?,
        data: serde_json::from_str(&candidate.data_json)?,
    })
}

/// The first of `candidate`'s stored records that fails its digest, if any.
pub(crate) fn content_incident(
    conn: &Connection,
    candidate: &CandidateRow,
) -> Result<Option<ContentIncident>, ReportingError> {
    for (record, json, sha256) in [
        (
            StoredRecord::ContentJson,
            &candidate.content_json,
            &candidate.content_sha256,
        ),
        (
            StoredRecord::DataJson,
            &candidate.data_json,
            &candidate.data_sha256,
        ),
    ] {
        if sha256_hex(json) != *sha256 {
            return Ok(Some(ContentIncident {
                subject: subject(conn, candidate)?,
                record,
                sha256: sha256.clone(),
            }));
        }
    }
    Ok(None)
}

fn subject(conn: &Connection, candidate: &CandidateRow) -> Result<IncidentSubject, ReportingError> {
    if candidate.state != CandidateState::Released {
        return Ok(IncidentSubject::Candidate(candidate.id));
    }
    let revision = reports::revision_of_candidate(conn, candidate.id)?
        .ok_or(ReportingError::Inconsistent("released candidate revision"))?;
    Ok(IncidentSubject::Revision(revision.id))
}
