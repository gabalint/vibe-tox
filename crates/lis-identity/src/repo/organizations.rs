//! SQL for client organizations, client locations and practitioners.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{ClientId, ClientLocationId, Npi, PractitionerId};
use crate::views::{ClientLocationView, ClientView, PractitionerView};

pub(crate) fn client_code_exists(conn: &Connection, code: &str) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM idn_client_organizations WHERE code = ?1)")?
        .query_row([code], |row| row.get(0))
}

pub(crate) fn client_exists(conn: &Connection, id: ClientId) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM idn_client_organizations WHERE id = ?1)")?
        .query_row([id], |row| row.get(0))
}

pub(crate) fn insert_client(
    conn: &Connection,
    id: ClientId,
    code: &str,
    name: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO idn_client_organizations (id, code, name, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![id, code, name, now])?;
    Ok(())
}

pub(crate) fn location_code_exists(
    conn: &Connection,
    client_id: ClientId,
    code: &str,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM idn_client_locations WHERE client_id = ?1 AND code = ?2)",
    )?
    .query_row(params![client_id, code], |row| row.get(0))
}

pub(crate) fn insert_location(
    conn: &Connection,
    id: ClientLocationId,
    client_id: ClientId,
    code: &str,
    name: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO idn_client_locations (id, client_id, code, name, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![id, client_id, code, name, now])?;
    Ok(())
}

pub(crate) fn npi_exists(conn: &Connection, npi: &Npi) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM idn_practitioners WHERE npi = ?1)")?
        .query_row([npi.as_str()], |row| row.get(0))
}

pub(crate) struct NewPractitioner<'a> {
    pub(crate) id: PractitionerId,
    pub(crate) npi: &'a Npi,
    pub(crate) family_name: &'a str,
    pub(crate) given_names: Option<&'a str>,
    pub(crate) created_at: UtcTimestamp,
}

pub(crate) fn insert_practitioner(
    conn: &Connection,
    new: &NewPractitioner<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO idn_practitioners (id, npi, family_name, given_names, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        new.id,
        new.npi.as_str(),
        new.family_name,
        new.given_names,
        new.created_at,
    ])?;
    Ok(())
}

fn client_view(row: &Row<'_>) -> rusqlite::Result<ClientView> {
    Ok(ClientView {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        locations: Vec::new(),
        active: row.get(3)?,
        revision: row.get(4)?,
    })
}

/// Clients by code, with empty location lists (see [`locations`]).
pub(crate) fn clients(conn: &Connection) -> rusqlite::Result<Vec<ClientView>> {
    conn.prepare_cached(
        "SELECT id, code, name, active, revision FROM idn_client_organizations ORDER BY code",
    )?
    .query_map([], client_view)?
    .collect()
}

/// One client, with an empty location list.
pub(crate) fn client(conn: &Connection, id: ClientId) -> rusqlite::Result<Option<ClientView>> {
    conn.prepare_cached(
        "SELECT id, code, name, active, revision FROM idn_client_organizations WHERE id = ?1",
    )?
    .query_row([id], client_view)
    .optional()
}

pub(crate) fn locations(
    conn: &Connection,
    client_id: ClientId,
) -> rusqlite::Result<Vec<ClientLocationView>> {
    conn.prepare_cached(
        "SELECT id, code, name, active, revision FROM idn_client_locations WHERE client_id = ?1 ORDER BY code",
    )?
    .query_map([client_id], |row| {
        Ok(ClientLocationView {
            id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            active: row.get(3)?,
            revision: row.get(4)?,
        })
    })?
    .collect()
}

fn practitioner_view(row: &Row<'_>) -> rusqlite::Result<PractitionerView> {
    Ok(PractitionerView {
        id: row.get(0)?,
        npi: Npi::from_stored(row.get(1)?),
        family_name: row.get(2)?,
        given_names: row.get(3)?,
        active: row.get(4)?,
        revision: row.get(5)?,
    })
}

pub(crate) fn practitioners(conn: &Connection) -> rusqlite::Result<Vec<PractitionerView>> {
    conn.prepare_cached(
        "SELECT id, npi, family_name, given_names, active, revision FROM idn_practitioners \
         ORDER BY family_name, id",
    )?
    .query_map([], practitioner_view)?
    .collect()
}

pub(crate) fn practitioner(
    conn: &Connection,
    id: PractitionerId,
) -> rusqlite::Result<Option<PractitionerView>> {
    conn.prepare_cached(
        "SELECT id, npi, family_name, given_names, active, revision FROM idn_practitioners WHERE id = ?1",
    )?
    .query_row([id], practitioner_view)
    .optional()
}
