//! SQL for this module's tables, one statement next to the function that
//! runs it, plus reads of other modules' published views.

pub(crate) mod au_profiles;
pub(crate) mod conflicts;
pub(crate) mod deviations;
pub(crate) mod lots;
pub(crate) mod published;
pub(crate) mod qc;
pub(crate) mod queues;
pub(crate) mod readiness;
pub(crate) mod runs;

/// Decode the first text column of a stored, versioned source configuration.
fn source_configuration<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<T> {
    let json: String = row.get(0)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}
