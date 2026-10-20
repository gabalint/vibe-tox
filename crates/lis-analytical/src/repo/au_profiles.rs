//! Immutable approved AU wire formats and declared numeric units.

use rusqlite::{Connection, OptionalExtension, params};

use crate::{AuSourceProfile, SourceRegistrationId};

pub(crate) fn insert(
    conn: &Connection,
    registration: SourceRegistrationId,
    profile: &AuSourceProfile,
) -> rusqlite::Result<()> {
    let json = serde_json::to_string(profile)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    conn.prepare_cached(
        "INSERT INTO ana_source_au_profiles (registration_id, profile_json, profile_version) \
         VALUES (?1, ?2, 1)",
    )?
    .execute(params![registration, json])?;
    Ok(())
}

pub(crate) fn get(
    conn: &Connection,
    registration: SourceRegistrationId,
) -> rusqlite::Result<Option<AuSourceProfile>> {
    conn.prepare_cached(
        "SELECT profile_json FROM ana_source_au_profiles \
         WHERE registration_id = ?1 AND profile_version = 1",
    )?
    .query_row([registration], super::source_configuration)
    .optional()
}
