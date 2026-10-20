//! Immutable performing-site attestation storage for previously unpinned runs.
use crate::model::RunId;
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

/// An immutable factual recovery for a previously unpinned run.
pub(crate) fn insert_site_resolution(
    conn: &Connection,
    input: &crate::RecordRunPerformingSiteInput,
    source: &str,
    reason: &str,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_run_performing_site_resolutions \
         (run_id, laboratory_profile_id, run_revision, source_reference, reason, recorded_by, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?.execute(params![input.run_id, input.laboratory_profile_id, input.expected_revision, source, reason, by, at])?;
    Ok(())
}

pub(crate) fn site_resolution(
    conn: &Connection,
    run_id: RunId,
) -> rusqlite::Result<Option<(lis_identity::LaboratoryProfileId, String, String)>> {
    conn.prepare_cached("SELECT laboratory_profile_id, source_reference, reason FROM ana_run_performing_site_resolutions WHERE run_id = ?1")?
        .query_row([run_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()
}
