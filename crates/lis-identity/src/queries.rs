//! Read-side use cases. They take a connection (a read connection, or the
//! caller's transaction for gate rechecks) and perform no authorization: the
//! engine checks the capability, and audits the read where the result holds
//! PHI, before calling them.

use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::error::IdentityError;
use crate::model::{
    ClientId, Demographics, DemographicsInput, IdentifierInput, PractitionerId, SubjectId,
    parse_identifiers,
};
use crate::repo::{holds, laboratory, organizations, subjects};
use crate::views::{
    ClientView, IdentityHoldRecord, IdentityHoldView, LaboratoryProfileView, PractitionerView,
    SubjectCandidates, SubjectMatches, SubjectSummary, SubjectView,
};

/// Current human label for an audit record, when this module owns one.
/// Labels are resolved at read time and never copied into audit storage.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, IdentityError> {
    match record_type {
        "subject" => {
            let Ok(id) = record_id.parse::<SubjectId>() else {
                return Ok(None);
            };
            subject_summary(conn, id)
                .map(|subject| {
                    Some(subject.given_names.as_ref().map_or_else(
                        || subject.family_name.expose().clone(),
                        |given| format!("{}, {}", subject.family_name.expose(), given.expose()),
                    ))
                })
                .or_else(identity_not_found_is_none)
        }
        "client" => {
            let Ok(id) = record_id.parse::<ClientId>() else {
                return Ok(None);
            };
            get_client(conn, id)
                .map(|client| Some(client.name))
                .or_else(identity_not_found_is_none)
        }
        "practitioner" => {
            let Ok(id) = record_id.parse::<PractitionerId>() else {
                return Ok(None);
            };
            get_practitioner(conn, id)
                .map(|practitioner| {
                    Some(practitioner.given_names.map_or_else(
                        || practitioner.family_name.clone(),
                        |given| format!("{}, {given}", practitioner.family_name),
                    ))
                })
                .or_else(identity_not_found_is_none)
        }
        _ => Ok(None),
    }
}

fn identity_not_found_is_none(error: IdentityError) -> Result<Option<String>, IdentityError> {
    match error {
        IdentityError::NotFound(_) => Ok(None),
        other @ (IdentityError::Invalid(_)
        | IdentityError::InvalidFinancialRecord
        | IdentityError::RevisionConflict
        | IdentityError::UserRequired
        | IdentityError::IdentifierInUse(_)
        | IdentityError::CandidatesNotAcknowledged
        | IdentityError::HoldResolved
        | IdentityError::IdentifierInactive
        | IdentityError::NothingChanged
        | IdentityError::Duplicate(_)
        | IdentityError::Store(_)) => Err(other),
    }
}

/// Input of [`find_subject_matches`] and [`match_subjects`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectMatchQuery {
    /// Demographics to list name and birth date candidates for.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub demographics: Option<DemographicsInput>,
    /// Identifiers to look up within their authority and type.
    #[serde(default)]
    pub identifiers: Vec<IdentifierInput>,
}

/// [`find_subject_matches`] with each subject summarized. PHI: an audited
/// read (capability `enter_orders`).
pub fn match_subjects(
    conn: &Connection,
    query: &SubjectMatchQuery,
) -> Result<SubjectCandidates, IdentityError> {
    let matches = find_subject_matches(conn, query)?;
    Ok(SubjectCandidates {
        identifier_matches: subject_summaries(conn, &matches.identifier_matches)?,
        candidates: subject_summaries(conn, &matches.candidates)?,
    })
}

/// Summaries of `subject_ids`, in the given order. PHI: callers audit the read.
pub fn subject_summaries(
    conn: &Connection,
    subject_ids: &[SubjectId],
) -> Result<Vec<SubjectSummary>, IdentityError> {
    subject_ids
        .iter()
        .map(|&subject_id| subject_summary(conn, subject_id))
        .collect()
}

/// A subject's current demographics, active identifiers and open hold count.
/// PHI: callers audit the read.
pub fn subject_summary(
    conn: &Connection,
    subject_id: SubjectId,
) -> Result<SubjectSummary, IdentityError> {
    let revision =
        subjects::subject_revision(conn, subject_id)?.ok_or(IdentityError::NotFound("subject"))?;
    let current =
        subjects::current_assertion(conn, subject_id)?.ok_or(IdentityError::NotFound("subject"))?;
    let mut identifiers = subjects::identifiers(conn, subject_id)?;
    identifiers.retain(|identifier| identifier.active);
    Ok(SubjectSummary {
        subject_id,
        revision,
        family_name: current.family_name,
        given_names: current.given_names,
        birth_date: current.birth_date,
        administrative_sex: current.administrative_sex,
        identifiers,
        open_identity_holds: holds::open_holds_touching(conn, Some(subject_id), None)?.len(),
    })
}

/// Deterministic matching (REQ-IDN-001): an active identifier within the same
/// authority and type is the same subject; otherwise subjects with the same
/// normalized name and birth date are listed as candidates only. Contains
/// PHI-derived results: an audited read (capability `enter_orders`).
pub fn find_subject_matches(
    conn: &Connection,
    query: &SubjectMatchQuery,
) -> Result<SubjectMatches, IdentityError> {
    let mut matches = SubjectMatches::default();
    for identifier in parse_identifiers(&query.identifiers)? {
        if let Some(owner) = subjects::active_identifier_owner(conn, &identifier)?
            && !matches.identifier_matches.contains(&owner)
        {
            matches.identifier_matches.push(owner);
        }
    }
    if let Some(input) = &query.demographics {
        let key = Demographics::parse(input)?.match_key();
        matches.candidates = subjects::subjects_by_match_key(conn, &key, None)?
            .into_iter()
            .filter(|subject| !matches.identifier_matches.contains(subject))
            .collect();
    }
    Ok(matches)
}

/// A subject with all assertions and identifiers. PHI: an audited read
/// (capability `enter_orders`).
pub fn get_subject(conn: &Connection, subject_id: SubjectId) -> Result<SubjectView, IdentityError> {
    let revision =
        subjects::subject_revision(conn, subject_id)?.ok_or(IdentityError::NotFound("subject"))?;
    Ok(SubjectView {
        id: subject_id,
        revision,
        assertions: subjects::assertions(conn, subject_id)?,
        identifiers: subjects::identifiers(conn, subject_id)?,
        open_holds: list_open_identity_holds(conn, subject_id)?,
    })
}

/// Open identity holds that block release for `subject_id`: holds on the
/// subject and holds listing it as a candidate (REQ-IDN-001). Empty means no
/// identity hold blocks. Release commands call this inside their own
/// transaction so the gate is rechecked at commit time.
pub fn list_open_identity_holds(
    conn: &Connection,
    subject_id: SubjectId,
) -> Result<Vec<IdentityHoldView>, IdentityError> {
    open_holds(conn, Some(subject_id), None)
}

/// The oldest `limit` open identity holds, for review and work queues, so a
/// backlog is never loaded whole. Holds name subjects by ID only, so this
/// read holds no PHI.
pub fn list_all_open_identity_holds(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<IdentityHoldView>, IdentityError> {
    open_holds(conn, None, Some(limit))
}

fn open_holds(
    conn: &Connection,
    subject_id: Option<SubjectId>,
    limit: Option<u32>,
) -> Result<Vec<IdentityHoldView>, IdentityError> {
    let mut open = holds::open_holds_touching(conn, subject_id, limit)?;
    for hold in &mut open {
        hold.candidate_subject_ids = holds::candidates(conn, hold.id)?;
    }
    Ok(open)
}

/// Client organizations with their locations, by code.
pub fn list_clients(conn: &Connection) -> Result<Vec<ClientView>, IdentityError> {
    let mut clients = organizations::clients(conn)?;
    for client in &mut clients {
        client.locations = organizations::locations(conn, client.id)?;
    }
    Ok(clients)
}

/// Practitioners by family name.
pub fn list_practitioners(conn: &Connection) -> Result<Vec<PractitionerView>, IdentityError> {
    Ok(organizations::practitioners(conn)?)
}

/// One client organization with its locations.
pub fn get_client(conn: &Connection, client_id: ClientId) -> Result<ClientView, IdentityError> {
    let mut client =
        organizations::client(conn, client_id)?.ok_or(IdentityError::NotFound("client"))?;
    client.locations = organizations::locations(conn, client_id)?;
    Ok(client)
}

/// One practitioner.
pub fn get_practitioner(
    conn: &Connection,
    practitioner_id: PractitionerId,
) -> Result<PractitionerView, IdentityError> {
    organizations::practitioner(conn, practitioner_id)?
        .ok_or(IdentityError::NotFound("practitioner"))
}

/// The current laboratory profile, or `None` before one is set. Laboratory
/// configuration without patient data: any signed-in user may read it.
pub fn current_laboratory_profile(
    conn: &Connection,
) -> Result<Option<LaboratoryProfileView>, IdentityError> {
    Ok(laboratory::current_profile(conn)?)
}

/// A frozen performing-site profile by identity, including superseded versions (RPT-05).
pub fn get_laboratory_profile_by_id(
    conn: &Connection,
    id: crate::model::LaboratoryProfileId,
) -> Result<LaboratoryProfileView, IdentityError> {
    laboratory::profile_by_id(conn, id)?.ok_or(IdentityError::NotFound("laboratory_profile"))
}

/// Frozen laboratory profiles for explicit performing-site recovery; newest version first.
pub fn list_laboratory_profiles(
    conn: &Connection,
) -> Result<Vec<LaboratoryProfileView>, IdentityError> {
    Ok(laboratory::profiles(conn)?)
}

/// Every identity hold on `subject_id` or listing it as a candidate, open
/// or resolved, oldest first (REQ-RPT-002 impact after release). Holds name
/// subjects by ID only.
pub fn identity_holds_touching(
    conn: &Connection,
    subject_id: SubjectId,
) -> Result<Vec<IdentityHoldRecord>, IdentityError> {
    let mut records = holds::holds_touching(conn, subject_id)?;
    for record in &mut records {
        record.hold.candidate_subject_ids = holds::candidates(conn, record.hold.id)?;
    }
    Ok(records)
}
