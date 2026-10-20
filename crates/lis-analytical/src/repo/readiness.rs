//! SQL for instrument status events and source registrations with their
//! test codes and file layouts.

use lis_catalog::AnalyteId;
use lis_core::{InstrumentId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::file_layout::FileLayout;
use crate::model::{
    InstrumentStatus, RegistrationState, RevisionIndicator, SampleIdField, SourceRegistrationId,
    StatusEventId,
};
use crate::views::{SourceRegistrationView, SourceTestCodeView};

pub(crate) fn insert_status_event(
    conn: &Connection,
    instrument_id: InstrumentId,
    status: InstrumentStatus,
    reason: &str,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<StatusEventId> {
    let id = StatusEventId::generate();
    conn.prepare_cached(
        "INSERT INTO ana_instrument_status_events \
         (id, instrument_id, status, reason, recorded_by, recorded_at, seq) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, (SELECT coalesce(max(seq), 0) + 1 \
         FROM ana_instrument_status_events WHERE instrument_id = ?2))",
    )?
    .execute(params![id, instrument_id, status, reason, by, at])?;
    Ok(id)
}

/// The instrument's latest status by sequence (never by the host clock) and
/// when it was recorded; `None` if never set.
pub(crate) fn latest_status(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> rusqlite::Result<Option<(InstrumentStatus, UtcTimestamp)>> {
    conn.prepare_cached(
        "SELECT status, recorded_at FROM ana_instrument_status_events WHERE instrument_id = ?1 \
         ORDER BY seq DESC LIMIT 1",
    )?
    .query_row([instrument_id], |row| Ok((row.get(0)?, row.get(1)?)))
    .optional()
}

pub(crate) struct NewRegistration<'a> {
    pub(crate) id: SourceRegistrationId,
    pub(crate) instrument_id: InstrumentId,
    pub(crate) sender_name: &'a str,
    pub(crate) sender_version: &'a str,
    pub(crate) sample_id_field: SampleIdField,
    pub(crate) revision_indicator: RevisionIndicator,
    pub(crate) reports_final_concentration: bool,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_registration(
    conn: &Connection,
    new: &NewRegistration<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_source_registrations (id, instrument_id, sender_name, sender_version, \
         sample_id_field, revision_indicator, reports_final_concentration, state, authored_by, \
         authored_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'draft', ?8, ?9, 1)",
    )?
    .execute(params![
        new.id,
        new.instrument_id,
        new.sender_name,
        new.sender_version,
        new.sample_id_field,
        new.revision_indicator,
        new.reports_final_concentration,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) fn insert_test_code(
    conn: &Connection,
    registration_id: SourceRegistrationId,
    code: &str,
    analyte_id: AnalyteId,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_source_test_codes (registration_id, source_test_code, analyte_id) \
         VALUES (?1, ?2, ?3)",
    )?
    .execute(params![registration_id, code, analyte_id])?;
    Ok(())
}

/// The current version of the stored layout JSON.
const LAYOUT_VERSION: i64 = 1;

pub(crate) fn insert_file_layout(
    conn: &Connection,
    registration_id: SourceRegistrationId,
    layout: &FileLayout,
) -> rusqlite::Result<()> {
    let json = serde_json::to_string(layout)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    conn.prepare_cached(
        "INSERT INTO ana_source_file_layouts (registration_id, layout_json, layout_version) \
         VALUES (?1, ?2, ?3)",
    )?
    .execute(params![registration_id, json, LAYOUT_VERSION])?;
    Ok(())
}

/// The registration's file layout; `None` for an ASTM source.
pub(crate) fn file_layout(
    conn: &Connection,
    registration_id: SourceRegistrationId,
) -> rusqlite::Result<Option<FileLayout>> {
    conn.prepare_cached(
        "SELECT layout_json FROM ana_source_file_layouts \
             WHERE registration_id = ?1 AND layout_version = ?2",
    )?
    .query_row(
        params![registration_id, LAYOUT_VERSION],
        super::source_configuration,
    )
    .optional()
}

pub(crate) fn activate(
    conn: &Connection,
    id: SourceRegistrationId,
    revision: i64,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_source_registrations SET state = 'active', approved_by = ?2, \
         approved_at = ?3, revision = ?4 WHERE id = ?1",
    )?
    .execute(params![id, by, at, revision])?;
    Ok(())
}

pub(crate) fn retire(
    conn: &Connection,
    id: SourceRegistrationId,
    revision: i64,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_source_registrations SET state = 'retired', retired_at = ?2, revision = ?3 \
         WHERE id = ?1",
    )?
    .execute(params![id, at, revision])?;
    Ok(())
}

const REGISTRATION_COLUMNS: &str = "id, instrument_id, sender_name, sender_version, \
     sample_id_field, revision_indicator, reports_final_concentration, state, authored_by, \
     authored_at, approved_by, approved_at, retired_at, revision";

fn registration_from_row(row: &Row<'_>) -> rusqlite::Result<SourceRegistrationView> {
    Ok(SourceRegistrationView {
        registration_id: row.get(0)?,
        instrument_id: row.get(1)?,
        sender_name: row.get(2)?,
        sender_version: row.get(3)?,
        sample_id_field: row.get(4)?,
        revision_indicator: row.get(5)?,
        reports_final_concentration: row.get(6)?,
        state: row.get(7)?,
        authored_by: row.get(8)?,
        authored_at: row.get(9)?,
        approved_by: row.get(10)?,
        approved_at: row.get(11)?,
        retired_at: row.get(12)?,
        revision: row.get(13)?,
        test_codes: Vec::new(),
        file_layout: None,
        au_profile: None,
        allowed_actions: Vec::new(),
    })
}

/// A registration without its test codes.
pub(crate) fn registration(
    conn: &Connection,
    id: SourceRegistrationId,
) -> rusqlite::Result<Option<SourceRegistrationView>> {
    conn.prepare_cached(&format!(
        "SELECT {REGISTRATION_COLUMNS} FROM ana_source_registrations WHERE id = ?1"
    ))?
    .query_row([id], registration_from_row)
    .optional()
}

/// The registration of `instrument_id` in `state` (one at most when active).
pub(crate) fn registrations(
    conn: &Connection,
    instrument_id: InstrumentId,
    state: Option<RegistrationState>,
) -> rusqlite::Result<Vec<SourceRegistrationView>> {
    conn.prepare_cached(&format!(
        "SELECT {REGISTRATION_COLUMNS} FROM ana_source_registrations \
         WHERE instrument_id = ?1 AND (?2 IS NULL OR state = ?2) ORDER BY authored_at DESC, id DESC"
    ))?
    .query_map(params![instrument_id, state], registration_from_row)?
    .collect()
}

pub(crate) fn test_codes(
    conn: &Connection,
    registration_id: SourceRegistrationId,
) -> rusqlite::Result<Vec<SourceTestCodeView>> {
    conn.prepare_cached(
        "SELECT source_test_code, analyte_id FROM ana_source_test_codes \
         WHERE registration_id = ?1 ORDER BY source_test_code",
    )?
    .query_map([registration_id], |row| {
        Ok(SourceTestCodeView {
            source_test_code: row.get(0)?,
            analyte_id: row.get(1)?,
        })
    })?
    .collect()
}
