//! SQL for laboratory profile versions.

use rusqlite::{Connection, OptionalExtension, params};

use crate::views::LaboratoryProfileView;

pub(crate) fn insert_profile(
    conn: &Connection,
    profile: &LaboratoryProfileView,
) -> rusqlite::Result<()> {
    let line = |index: usize| profile.address_lines.get(index).map(String::as_str);
    conn.prepare_cached(
        "INSERT INTO idn_laboratory_profiles (id, version, name, address_line_1, address_line_2, \
         address_line_3, clia_id, director_name, phone, time_zone, billing_npi, tax_id, reason, \
         set_by, set_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    )?
    .execute(params![
        profile.profile_id,
        profile.version,
        profile.name,
        line(0),
        line(1),
        line(2),
        profile.clia_id,
        profile.director,
        profile.phone,
        profile.time_zone,
        profile.billing_npi,
        profile.tax_id,
        profile.reason,
        profile.set_by,
        profile.set_at,
    ])?;
    Ok(())
}

/// The highest version.
pub(crate) fn current_profile(
    conn: &Connection,
) -> rusqlite::Result<Option<LaboratoryProfileView>> {
    conn.prepare_cached(
        "SELECT id, version, name, address_line_1, address_line_2, address_line_3, clia_id, \
         director_name, phone, time_zone, billing_npi, tax_id, reason, set_by, set_at \
         FROM idn_laboratory_profiles ORDER BY version DESC LIMIT 1",
    )?
    .query_row([], profile_row)
    .optional()
}

pub(crate) fn profile_by_id(
    conn: &Connection,
    id: crate::model::LaboratoryProfileId,
) -> rusqlite::Result<Option<LaboratoryProfileView>> {
    conn.prepare_cached(
        "SELECT id, version, name, address_line_1, address_line_2, address_line_3, clia_id, \
         director_name, phone, time_zone, billing_npi, tax_id, reason, set_by, set_at \
         FROM idn_laboratory_profiles WHERE id = ?1",
    )?
    .query_row([id], profile_row)
    .optional()
}

fn profile_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LaboratoryProfileView> {
    let lines: [Option<String>; 3] = [row.get(3)?, row.get(4)?, row.get(5)?];
    Ok(LaboratoryProfileView {
        profile_id: row.get(0)?,
        version: row.get(1)?,
        name: row.get(2)?,
        address_lines: lines.into_iter().flatten().collect(),
        clia_id: row.get(6)?,
        director: row.get(7)?,
        phone: row.get(8)?,
        time_zone: row.get(9)?,
        billing_npi: row.get(10)?,
        tax_id: row.get(11)?,
        reason: row.get(12)?,
        set_by: row.get(13)?,
        set_at: row.get(14)?,
    })
}

pub(crate) fn profiles(conn: &Connection) -> rusqlite::Result<Vec<LaboratoryProfileView>> {
    conn.prepare_cached(
        "SELECT id, version, name, address_line_1, address_line_2, address_line_3, clia_id, \
         director_name, phone, time_zone, billing_npi, tax_id, reason, set_by, set_at \
         FROM idn_laboratory_profiles ORDER BY version DESC",
    )?
    .query_map([], profile_row)?
    .collect()
}
