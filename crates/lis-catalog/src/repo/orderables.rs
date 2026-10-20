//! SQL for orderable versions and their component manifests.

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::content::OrderableVersionContent;
use crate::model::{OrderableId, OrderableVersionId};
use crate::repo::lifecycle::{LIFECYCLE_COLUMNS, lifecycle_at};
use crate::views::{ComponentAnalyteView, ComponentView, OrderableVersionView};

pub(crate) fn insert_version(
    conn: &Connection,
    id: OrderableVersionId,
    orderable_id: OrderableId,
    version: i64,
    created_by: UserId,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_orderable_versions (id, orderable_id, version, status, revision, \
         created_by, created_at) VALUES (?1, ?2, ?3, 'draft', 1, ?4, ?5)",
    )?
    .execute(params![id, orderable_id, version, created_by, now])?;
    Ok(())
}

/// Writes the manifest of a draft; positions are numbered from 1.
pub(crate) fn insert_components(
    conn: &Connection,
    id: OrderableVersionId,
    content: &OrderableVersionContent,
) -> rusqlite::Result<()> {
    let mut component = conn.prepare_cached(
        "INSERT INTO cat_orderable_components (orderable_version_id, position, \
         method_version_id, purpose) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut analyte = conn.prepare_cached(
        "INSERT INTO cat_orderable_component_analytes (orderable_version_id, position, \
         method_version_id, analyte_id) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (position, input) in (1_i64..).zip(&content.components) {
        component.execute(params![
            id,
            position,
            input.method_version_id,
            input.purpose
        ])?;
        for analyte_id in &input.analyte_ids {
            analyte.execute(params![id, position, input.method_version_id, analyte_id])?;
        }
    }
    Ok(())
}

/// Removes a draft's manifest. Triggers refuse this for an approved or retired version.
pub(crate) fn delete_components(conn: &Connection, id: OrderableVersionId) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "DELETE FROM cat_orderable_component_analytes WHERE orderable_version_id = ?1",
    )?
    .execute([id])?;
    conn.prepare_cached("DELETE FROM cat_orderable_components WHERE orderable_version_id = ?1")?
        .execute([id])?;
    Ok(())
}

/// Number of components whose method version is not current approved (REQ-ANA-009).
pub(crate) fn components_not_current(
    conn: &Connection,
    id: OrderableVersionId,
) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "SELECT count(*) FROM cat_orderable_components c \
         JOIN cat_method_versions m ON m.id = c.method_version_id \
         WHERE c.orderable_version_id = ?1 \
         AND NOT (m.status = 'approved' AND m.effective_to IS NULL)",
    )?
    .query_row([id], |row| row.get(0))
}

/// IDs of the current approved orderable versions, by orderable code.
pub(crate) fn current_version_ids(conn: &Connection) -> rusqlite::Result<Vec<OrderableVersionId>> {
    conn.prepare_cached(
        "SELECT v.id FROM cat_orderable_versions v JOIN cat_orderables o ON o.id = v.orderable_id \
         WHERE v.status = 'approved' AND v.effective_to IS NULL ORDER BY o.code",
    )?
    .query_map([], |row| row.get(0))?
    .collect()
}

/// The version with empty components (see [`components`]).
pub(crate) fn version(
    conn: &Connection,
    id: OrderableVersionId,
) -> rusqlite::Result<Option<OrderableVersionView>> {
    let sql = format!(
        "SELECT v.id, v.orderable_id, o.code, o.name, o.kind, v.version, {LIFECYCLE_COLUMNS} \
         FROM cat_orderable_versions v JOIN cat_orderables o ON o.id = v.orderable_id \
         WHERE v.id = ?1"
    );
    conn.prepare_cached(&sql)?
        .query_row([id], |row| {
            Ok(OrderableVersionView {
                id: row.get(0)?,
                orderable_id: row.get(1)?,
                code: row.get(2)?,
                name: row.get(3)?,
                kind: row.get(4)?,
                version: row.get(5)?,
                lifecycle: lifecycle_at(row, 6)?,
                components: Vec::new(),
            })
        })
        .optional()
}

/// Components by position, with empty analytes (see [`component_analytes`]).
pub(crate) fn components(
    conn: &Connection,
    id: OrderableVersionId,
) -> rusqlite::Result<Vec<ComponentView>> {
    conn.prepare_cached(
        "SELECT c.position, c.purpose, c.method_version_id, m.code, mv.version, s.code, \
         mv.certainty, mv.output_scale FROM cat_orderable_components c \
         JOIN cat_method_versions mv ON mv.id = c.method_version_id \
         JOIN cat_methods m ON m.id = mv.method_id \
         JOIN cat_specimen_types s ON s.id = mv.specimen_type_id \
         WHERE c.orderable_version_id = ?1 ORDER BY c.position",
    )?
    .query_map([id], |row| {
        Ok(ComponentView {
            position: row.get(0)?,
            purpose: row.get(1)?,
            method_version_id: row.get(2)?,
            method_code: row.get(3)?,
            method_version: row.get(4)?,
            specimen_type_code: row.get(5)?,
            certainty: row.get(6)?,
            output_scale: row.get(7)?,
            analytes: Vec::new(),
        })
    })?
    .collect()
}

pub(crate) fn component_analytes(
    conn: &Connection,
    id: OrderableVersionId,
    position: i64,
) -> rusqlite::Result<Vec<ComponentAnalyteView>> {
    conn.prepare_cached(
        "SELECT a.id, a.code, a.name, a.loinc_code, a.drug_class_code \
         FROM cat_orderable_component_analytes x JOIN cat_analytes a ON a.id = x.analyte_id \
         WHERE x.orderable_version_id = ?1 AND x.position = ?2 ORDER BY a.code",
    )?
    .query_map(params![id, position], |row| {
        Ok(ComponentAnalyteView {
            analyte_id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            loinc_code: row.get(3)?,
            drug_class_code: row.get(4)?,
        })
    })?
    .collect()
}
