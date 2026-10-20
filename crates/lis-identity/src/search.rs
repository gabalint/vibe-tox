//! Patient directory input validation and bounded search (REQ-IDN-001).

use lis_core::Phi;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::error::{IdentityError, invalid};
use crate::model::{
    Identifier, IdentifierInput, MATCH_KEY_SEPARATOR, NAME_MAX, birth_date, name_key, optional_text,
};
use crate::queries::subject_summaries;
use crate::repo::{search::PatientSearch, subjects};
use crate::views::SubjectSummary;

/// Input of [`search_subjects`]: patient directory browsing or a bounded search.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectSearch {
    /// Literal prefix of a family name, given names, full name (Given Family,
    /// Family Given or Family, Given), or active identifier value. Names ignore
    /// case and Unicode composition; identifier values are case-sensitive.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub query: Option<Phi<String>>,
    /// Number of matching patients to skip; defaults to zero.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub offset: Option<u32>,
    /// An identifier, matched exactly within its authority and type.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub identifier: Option<IdentifierInput>,
    /// The start of a family name, or the whole family name when given names
    /// are also entered; case and Unicode composition are ignored.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub family_name: Option<Phi<String>>,
    /// The start of the given names.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub given_names: Option<Phi<String>>,
    /// The exact birth date (`YYYY-MM-DD`).
    #[serde(default)]
    #[ts(optional = nullable)]
    pub birth_date: Option<Phi<String>>,
    /// Most subjects to return, 1–50.
    pub limit: u32,
}

/// Browses or finds patients for order entry (REQ-IDN-001), at most `limit`.
/// Blank criteria browse all patients. Search includes former names; summaries
/// and alphabetical ordering use current demographics, then the subject ID.
/// An exact scoped identifier match comes first. PHI: audited by the engine
/// under `enter_orders`; search never establishes an identity match.
pub fn search_subjects(
    conn: &Connection,
    search: &SubjectSearch,
) -> Result<Vec<SubjectSummary>, IdentityError> {
    if !(1..=50).contains(&search.limit) {
        return Err(invalid(
            "limit",
            "out_of_range",
            "Use a limit from 1 to 50.",
        ));
    }
    let identifier = search
        .identifier
        .as_ref()
        .map(|input| Identifier::parse("identifier", input))
        .transpose()?;
    let owner = identifier
        .as_ref()
        .map(|identifier| subjects::active_identifier_owner(conn, identifier))
        .transpose()?
        .flatten();
    let query = optional_text(
        "query",
        search.query.as_ref().map(|query| query.expose().as_str()),
        NAME_MAX * 2 + 2,
    )?;
    let birth = search
        .birth_date
        .as_ref()
        .map(|date| birth_date(date.expose()).map(|date| Phi::new(date.to_owned())))
        .transpose()?;
    let criteria = PatientSearch {
        identifier_owner: owner,
        has_identifier: identifier.is_some(),
        name_prefix: name_prefix(search)?,
        query: query.map(Phi::new),
        birth_date: birth,
        limit: search.limit,
        offset: search.offset.unwrap_or(0),
    };
    let found = crate::repo::search::patients(conn, &criteria)?;
    subject_summaries(conn, &found)
}

/// The match-key prefix of a name search: the family name start, or the whole
/// family name and the given names start.
fn name_prefix(search: &SubjectSearch) -> Result<Option<Phi<String>>, IdentityError> {
    let family = optional_text(
        "familyName",
        search
            .family_name
            .as_ref()
            .map(|name| name.expose().as_str()),
        NAME_MAX,
    )?;
    let given = optional_text(
        "givenNames",
        search
            .given_names
            .as_ref()
            .map(|name| name.expose().as_str()),
        NAME_MAX,
    )?;
    Ok(match (family, given) {
        (None, None) => None,
        (None, Some(_)) => {
            return Err(invalid(
                "familyName",
                "required",
                "Enter the family name to search by given names.",
            ));
        }
        (Some(family), None) => Some(Phi::new(name_key(&family))),
        (Some(family), Some(given)) => Some(Phi::new(format!(
            "{}{MATCH_KEY_SEPARATOR}{}",
            name_key(&family),
            name_key(&given)
        ))),
    })
}
