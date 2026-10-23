//! SQL for container holds and storage excursions.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{
    ContainerHoldKind, ContainerHoldState, ContainerId, ExcursionId, ExcursionOutcome,
    ExcursionState, HoldId, LocationId, SpecimenId,
};
use crate::views::{ContainerHoldRecord, ContainerHoldView, ExcursionView};

pub(crate) struct NewHold<'a> {
    pub(crate) id: HoldId,
    pub(crate) container_id: ContainerId,
    pub(crate) kind: ContainerHoldKind,
    pub(crate) excursion_id: Option<ExcursionId>,
    pub(crate) reason: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_hold(conn: &Connection, new: &NewHold<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_holds (id, container_id, kind, excursion_id, reason, state, placed_by, \
         placed_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, 1)",
    )?
    .execute(params![
        new.id,
        new.container_id,
        new.kind,
        new.excursion_id,
        new.reason,
        new.by,
        new.at
    ])?;
    Ok(())
}

const HOLD_COLUMNS: &str =
    "id, container_id, kind, state, excursion_id, reason, placed_at, revision FROM spc_holds";

fn hold_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContainerHoldView> {
    Ok(ContainerHoldView {
        hold_id: row.get(0)?,
        container_id: row.get(1)?,
        kind: row.get(2)?,
        state: row.get(3)?,
        excursion_id: row.get(4)?,
        reason: row.get(5)?,
        placed_at: row.get(6)?,
        revision: row.get(7)?,
    })
}

pub(crate) fn hold(conn: &Connection, id: HoldId) -> rusqlite::Result<Option<ContainerHoldView>> {
    conn.prepare_cached(&format!("SELECT {HOLD_COLUMNS} WHERE id = ?1"))?
        .query_row([id], hold_view)
        .optional()
}

pub(crate) fn open_holds(
    conn: &Connection,
    container_id: ContainerId,
) -> rusqlite::Result<Vec<ContainerHoldView>> {
    conn.prepare_cached(&format!(
        "SELECT {HOLD_COLUMNS} WHERE container_id = ?1 AND state = 'open' ORDER BY placed_at, id"
    ))?
    .query_map([container_id], hold_view)?
    .collect()
}

/// The kinds of the open holds on any container of a specimen.
pub(crate) fn open_hold_kinds(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<Vec<ContainerHoldKind>> {
    conn.prepare_cached(
        "SELECT DISTINCT h.kind FROM spc_holds h JOIN spc_containers c ON c.id = h.container_id \
         WHERE c.specimen_id = ?1 AND h.state = 'open' ORDER BY h.kind",
    )?
    .query_map([specimen_id], |row| row.get(0))?
    .collect()
}

/// Every hold ever placed on a specimen's containers, open or released.
pub(crate) fn specimen_hold_history(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<Vec<ContainerHoldRecord>> {
    conn.prepare_cached(
        "SELECT h.id, h.container_id, h.kind, h.state, h.excursion_id, h.reason, h.placed_at, \
         h.revision, h.released_at FROM spc_holds h \
         JOIN spc_containers c ON c.id = h.container_id \
         WHERE c.specimen_id = ?1 ORDER BY h.placed_at, h.id",
    )?
    .query_map([specimen_id], |row| {
        Ok(ContainerHoldRecord {
            hold: hold_view(row)?,
            released_at: row.get(8)?,
        })
    })?
    .collect()
}

pub(crate) fn excursion_holds(
    conn: &Connection,
    excursion_id: ExcursionId,
) -> rusqlite::Result<Vec<ContainerHoldView>> {
    conn.prepare_cached(&format!(
        "SELECT {HOLD_COLUMNS} WHERE excursion_id = ?1 ORDER BY container_id"
    ))?
    .query_map([excursion_id], hold_view)?
    .collect()
}

pub(crate) fn release_hold(
    conn: &Connection,
    id: HoldId,
    reason: &str,
    by: &str,
    at: UtcTimestamp,
    revision: i64,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE spc_holds SET state = ?2, released_by = ?3, released_at = ?4, \
         release_reason = ?5, revision = ?6 WHERE id = ?1",
    )?
    .execute(params![
        id,
        ContainerHoldState::Released,
        by,
        at,
        reason,
        revision
    ])?;
    Ok(())
}

pub(crate) struct NewExcursion<'a> {
    pub(crate) id: ExcursionId,
    pub(crate) location_id: LocationId,
    pub(crate) observed_from: UtcTimestamp,
    pub(crate) observed_to: Option<UtcTimestamp>,
    pub(crate) description: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_excursion(conn: &Connection, new: &NewExcursion<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_excursions (id, location_id, observed_from, observed_to, description, \
         recorded_by, recorded_at, state, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', 1)",
    )?
    .execute(params![
        new.id,
        new.location_id,
        new.observed_from,
        new.observed_to,
        new.description,
        new.by,
        new.at
    ])?;
    Ok(())
}

/// An excursion without its holds.
pub(crate) fn excursion(
    conn: &Connection,
    id: ExcursionId,
) -> rusqlite::Result<Option<ExcursionView>> {
    conn.prepare_cached(
        "SELECT id, location_id, observed_from, observed_to, description, state, outcome, \
         revision, recorded_by, recorded_at, assessed_by, assessed_at, assessment_reason FROM spc_excursions WHERE id = ?1",
    )?
    .query_row([id], |row| {
        Ok(ExcursionView {
            excursion_id: row.get(0)?,
            location_id: row.get(1)?,
            observed_from: row.get(2)?,
            observed_to: row.get(3)?,
            description: row.get(4)?,
            state: row.get(5)?,
            outcome: row.get(6)?,
            revision: row.get(7)?,
            affected_container_labels: std::collections::BTreeMap::new(),
            recorded_by: row.get(8)?,
            recorded_by_display: None,
            recorded_at: row.get(9)?,
            assessed_by: row.get(10)?,
            assessed_by_display: None,
            assessed_at: row.get(11)?,
            assessment_reason: row.get(12)?,
            holds: Vec::new(),
        })
    })
    .optional()
}

pub(crate) struct Assessment<'a> {
    pub(crate) id: ExcursionId,
    pub(crate) outcome: ExcursionOutcome,
    pub(crate) reason: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
    pub(crate) revision: i64,
}

pub(crate) fn assess_excursion(
    conn: &Connection,
    assessment: &Assessment<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE spc_excursions SET state = ?2, outcome = ?3, assessed_by = ?4, assessed_at = ?5, \
         assessment_reason = ?6, revision = ?7 WHERE id = ?1",
    )?
    .execute(params![
        assessment.id,
        ExcursionState::Assessed,
        assessment.outcome,
        assessment.by,
        assessment.at,
        assessment.reason,
        assessment.revision
    ])?;
    Ok(())
}
