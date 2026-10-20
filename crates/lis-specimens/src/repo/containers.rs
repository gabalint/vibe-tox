//! SQL for containers, moves and disposals.

use lis_core::{DecimalText, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::input::Volume;
use crate::model::{
    AccessionId, ContainerId, ContainerKind, ContainerState, LocationEventId, LocationId,
    SpecimenAcceptance, SpecimenId, VolumeKind,
};
use crate::views::{ContainerForTesting, ContainerView, StorageLocationRef};

pub(crate) struct NewContainer<'a> {
    pub(crate) id: ContainerId,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) label_code: &'a str,
    pub(crate) parent: Option<ContainerId>,
    pub(crate) volume: &'a Volume,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert_container(conn: &Connection, new: &NewContainer<'_>) -> rusqlite::Result<()> {
    let kind = if new.parent.is_some() {
        ContainerKind::Aliquot
    } else {
        ContainerKind::Primary
    };
    let (amount, unit) = new
        .volume
        .amount
        .as_ref()
        .map(|(amount, unit)| (amount, unit.as_str()))
        .unzip();
    conn.prepare_cached(
        "INSERT INTO spc_containers (id, specimen_id, label_code, kind, parent_container_id, \
         volume, volume_unit, volume_kind, state, created_by, created_at, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available', ?9, ?10, 1)",
    )?
    .execute(params![
        new.id,
        new.specimen_id,
        new.label_code,
        kind,
        new.parent,
        amount,
        unit,
        new.volume.kind,
        new.by,
        new.at
    ])?;
    Ok(())
}

/// A container with what its gates need: specimen acceptance and open holds.
pub(crate) struct ContainerRow {
    pub(crate) id: ContainerId,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) state: ContainerState,
    pub(crate) volume: Option<(DecimalText, String)>,
    pub(crate) revision: i64,
    pub(crate) acceptance: SpecimenAcceptance,
    pub(crate) open_holds: i64,
}

pub(crate) fn container(
    conn: &Connection,
    id: ContainerId,
) -> rusqlite::Result<Option<ContainerRow>> {
    conn.prepare_cached(
        "SELECT c.id, c.specimen_id, c.state, c.volume, c.volume_unit, c.revision, s.acceptance, \
         (SELECT count(*) FROM spc_holds h WHERE h.container_id = c.id AND h.state = 'open') \
         FROM spc_containers c JOIN spc_specimens s ON s.id = c.specimen_id WHERE c.id = ?1",
    )?
    .query_row([id], |row| {
        let amount: Option<DecimalText> = row.get(3)?;
        let unit: Option<String> = row.get(4)?;
        Ok(ContainerRow {
            id: row.get(0)?,
            specimen_id: row.get(1)?,
            state: row.get(2)?,
            volume: amount.zip(unit),
            revision: row.get(5)?,
            acceptance: row.get(6)?,
            open_holds: row.get(7)?,
        })
    })
    .optional()
}

/// The volumes taken by a container's aliquots; `None` where unknown.
pub(crate) fn taken_volumes(
    conn: &Connection,
    parent: ContainerId,
) -> rusqlite::Result<Vec<Option<(DecimalText, String)>>> {
    conn.prepare_cached(
        "SELECT volume, volume_unit FROM spc_containers WHERE parent_container_id = ?1",
    )?
    .query_map([parent], |row| {
        let amount: Option<DecimalText> = row.get(0)?;
        let unit: Option<String> = row.get(1)?;
        Ok(amount.zip(unit))
    })?
    .collect()
}

pub(crate) fn update_state(
    conn: &Connection,
    id: ContainerId,
    state: ContainerState,
    revision: i64,
) -> rusqlite::Result<()> {
    conn.prepare_cached("UPDATE spc_containers SET state = ?2, revision = ?3 WHERE id = ?1")?
        .execute(params![id, state, revision])?;
    Ok(())
}

pub(crate) fn insert_move(
    conn: &Connection,
    container_id: ContainerId,
    location_id: LocationId,
    reason: Option<&str>,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_location_events (id, container_id, location_id, moved_by, moved_at, \
         reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        LocationEventId::generate(),
        container_id,
        location_id,
        by,
        at,
        reason
    ])?;
    Ok(())
}

pub(crate) fn insert_disposal(
    conn: &Connection,
    container_id: ContainerId,
    reason: &str,
    by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_disposals (container_id, disposed_by, disposed_at, reason) \
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![container_id, by, at, reason])?;
    Ok(())
}

const VIEW_COLUMNS: &str = "c.id, c.label_code, c.specimen_id, c.kind, c.parent_container_id, \
    c.state, c.volume, c.volume_unit, c.volume_kind, c.revision, l.id, l.code, l.name, \
    (EXISTS(SELECT 1 FROM spc_location_events prior WHERE prior.container_id = c.id) AND e.id IS NULL)";
/// Containers with their latest location.
const VIEW_FROM: &str = "FROM spc_containers c \
    LEFT JOIN spc_location_events e ON e.id = (SELECT event_id FROM spc_location_order \
        WHERE container_id = c.id AND (source = 'command' OR NOT EXISTS \
          (SELECT 1 FROM spc_location_events legacy LEFT JOIN spc_location_order linked \
           ON linked.event_id = legacy.id WHERE legacy.container_id = c.id AND linked.event_id IS NULL)) \
        ORDER BY sequence DESC LIMIT 1) \
    LEFT JOIN spc_locations l ON l.id = e.location_id";

/// A container view without holds, label jobs or allowed actions.
fn container_view(row: &Row<'_>) -> rusqlite::Result<ContainerView> {
    let location_id: Option<LocationId> = row.get(10)?;
    let location = match location_id {
        Some(location_id) => Some(StorageLocationRef {
            location_id,
            code: row.get(11)?,
            name: row.get(12)?,
        }),
        None => None,
    };
    let volume_kind: VolumeKind = row.get(8)?;
    Ok(ContainerView {
        container_id: row.get(0)?,
        label_code: row.get(1)?,
        specimen_id: row.get(2)?,
        kind: row.get(3)?,
        parent_container_id: row.get(4)?,
        state: row.get(5)?,
        volume: row.get(6)?,
        volume_unit: row.get(7)?,
        volume_kind,
        revision: row.get(9)?,
        location,
        location_chronology_unresolved: row.get(13)?,
        open_holds: Vec::new(),
        label_jobs: Vec::new(),
        allowed_actions: Vec::new(),
    })
}

/// The containers of a specimen, in creation order.
pub(crate) fn specimen_containers(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<Vec<ContainerView>> {
    conn.prepare_cached(&format!(
        "SELECT {VIEW_COLUMNS} {VIEW_FROM} WHERE c.specimen_id = ?1 ORDER BY c.label_code"
    ))?
    .query_map([specimen_id], container_view)?
    .collect()
}

/// A container found by label code, with its specimen and accession.
pub(crate) struct LabelledContainer {
    pub(crate) view: ContainerView,
    pub(crate) acceptance: SpecimenAcceptance,
    pub(crate) specimen_type_code: String,
    pub(crate) accession_id: AccessionId,
}

pub(crate) fn container_by_label(
    conn: &Connection,
    label_code: &str,
) -> rusqlite::Result<Option<LabelledContainer>> {
    conn.prepare_cached(&format!(
        "SELECT {VIEW_COLUMNS}, s.acceptance, s.specimen_type_code, col.accession_id \
         {VIEW_FROM} JOIN spc_specimens s ON s.id = c.specimen_id \
         JOIN spc_collections col ON col.id = s.collection_id WHERE c.label_code = ?1"
    ))?
    .query_row([label_code], |row| {
        Ok(LabelledContainer {
            view: container_view(row)?,
            acceptance: row.get(14)?,
            specimen_type_code: row.get(15)?,
            accession_id: row.get(16)?,
        })
    })
    .optional()
}

/// A container with what the testing gate checks, from `spc_v_containers`.
pub(crate) struct TestingRow {
    pub(crate) container: ContainerForTesting,
    pub(crate) specimen_type_code: String,
    pub(crate) acceptance: SpecimenAcceptance,
    pub(crate) state: ContainerState,
    pub(crate) open_holds: i64,
}

pub(crate) fn testing_row(
    conn: &Connection,
    container_id: ContainerId,
) -> rusqlite::Result<Option<TestingRow>> {
    conn.prepare_cached(
        "SELECT container_id, label_code, specimen_id, accession_id, subject_id, \
         specimen_type_code, acceptance, state, open_hold_count FROM spc_v_containers \
         WHERE container_id = ?1",
    )?
    .query_row([container_id], |row| {
        Ok(TestingRow {
            container: ContainerForTesting {
                container_id: row.get(0)?,
                label_code: row.get(1)?,
                specimen_id: row.get(2)?,
                accession_id: row.get(3)?,
                subject_id: row.get(4)?,
            },
            specimen_type_code: row.get(5)?,
            acceptance: row.get(6)?,
            state: row.get(7)?,
            open_holds: row.get(8)?,
        })
    })
    .optional()
}
