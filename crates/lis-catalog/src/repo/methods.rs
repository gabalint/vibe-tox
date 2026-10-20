//! SQL for method version content and analyte limits.

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::content::{MethodAnalyte, MethodVersionContent};
use crate::model::{AnalyteId, MethodId, MethodVersionId, OrderableId};
use crate::repo::lifecycle::{LIFECYCLE_COLUMNS, lifecycle_at};
use crate::views::{MethodAnalyteView, MethodVersionView};

pub(crate) struct NewMethodVersion<'a> {
    pub(crate) id: MethodVersionId,
    pub(crate) method_id: MethodId,
    pub(crate) version: i64,
    pub(crate) content: &'a MethodVersionContent,
    pub(crate) created_by: UserId,
    pub(crate) created_at: UtcTimestamp,
}

pub(crate) fn insert_version(
    conn: &Connection,
    new: &NewMethodVersion<'_>,
) -> rusqlite::Result<()> {
    let c = new.content;
    conn.prepare_cached(
        "INSERT INTO cat_method_versions (id, method_id, version, status, revision, \
         specimen_type_id, certainty, output_scale, threshold_rule, threshold_rounding, \
         created_by, created_at, report_notes) VALUES (?1, ?2, ?3, 'draft', 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?
    .execute(params![
        new.id,
        new.method_id,
        new.version,
        c.specimen_type_id,
        c.certainty,
        c.output_scale,
        c.threshold_rule,
        c.threshold_rounding,
        new.created_by,
        new.created_at,
        c.report_notes,
    ])?;
    insert_analytes(conn, new.id, &c.analytes)
}

/// Replaces a draft's content. Triggers refuse this for an approved or
/// retired version. Kept analytes are updated in place, so orderable drafts
/// that reference them stay valid; the caller refuses removing an analyte an
/// orderable draft expects.
pub(crate) fn replace_content(
    conn: &Connection,
    id: MethodVersionId,
    content: &MethodVersionContent,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE cat_method_versions SET specimen_type_id = ?2, certainty = ?3, \
         output_scale = ?4, threshold_rule = ?5, threshold_rounding = ?6, report_notes = ?7 WHERE id = ?1",
    )?
    .execute(params![
        id,
        content.specimen_type_id,
        content.certainty,
        content.output_scale,
        content.threshold_rule,
        content.threshold_rounding,
        content.report_notes,
    ])?;
    let stored: Vec<AnalyteId> = conn
        .prepare_cached(
            "SELECT analyte_id FROM cat_method_version_analytes WHERE method_version_id = ?1",
        )?
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut delete = conn.prepare_cached(
        "DELETE FROM cat_method_version_analytes WHERE method_version_id = ?1 AND analyte_id = ?2",
    )?;
    for analyte in stored {
        if !content.analytes.iter().any(|a| a.analyte_id == analyte) {
            delete.execute(params![id, analyte])?;
        }
    }
    insert_analytes(conn, id, &content.analytes)
}

/// Inserts each analyte's limits, or replaces them when the row exists.
fn insert_analytes(
    conn: &Connection,
    id: MethodVersionId,
    analytes: &[MethodAnalyte],
) -> rusqlite::Result<()> {
    let mut statement = conn.prepare_cached(
        "INSERT INTO cat_method_version_analytes (method_version_id, analyte_id, unit, cutoff, \
         limit_of_detection, limit_of_quantitation, reportable_low, reportable_high, \
         display_scale, reference_interval, range_policy) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
         ON CONFLICT (method_version_id, analyte_id) DO UPDATE SET unit = excluded.unit, \
         cutoff = excluded.cutoff, limit_of_detection = excluded.limit_of_detection, \
         limit_of_quantitation = excluded.limit_of_quantitation, \
         reportable_low = excluded.reportable_low, reportable_high = excluded.reportable_high, \
         display_scale = excluded.display_scale, reference_interval = excluded.reference_interval, range_policy = excluded.range_policy",
    )?;
    for a in analytes {
        statement.execute(params![
            id,
            a.analyte_id,
            a.unit,
            a.cutoff,
            a.limit_of_detection,
            a.limit_of_quantitation,
            a.reportable_low,
            a.reportable_high,
            a.display_scale,
            a.reference_interval,
            a.range_policy,
        ])?;
    }
    Ok(())
}

/// The version with empty analytes (see [`analytes`]).
pub(crate) fn version(
    conn: &Connection,
    id: MethodVersionId,
) -> rusqlite::Result<Option<MethodVersionView>> {
    let sql = format!(
        "SELECT v.id, v.method_id, m.code, m.name, v.version, v.specimen_type_id, s.code, \
         v.certainty, v.output_scale, v.threshold_rule, v.threshold_rounding, \
         {LIFECYCLE_COLUMNS}, v.report_notes \
         FROM cat_method_versions v JOIN cat_methods m ON m.id = v.method_id \
         JOIN cat_specimen_types s ON s.id = v.specimen_type_id WHERE v.id = ?1"
    );
    conn.prepare_cached(&sql)?
        .query_row([id], |row| {
            Ok(MethodVersionView {
                id: row.get(0)?,
                method_id: row.get(1)?,
                method_code: row.get(2)?,
                method_name: row.get(3)?,
                version: row.get(4)?,
                specimen_type_id: row.get(5)?,
                specimen_type_code: row.get(6)?,
                certainty: row.get(7)?,
                output_scale: row.get(8)?,
                threshold_rule: row.get(9)?,
                threshold_rounding: row.get(10)?,
                lifecycle: lifecycle_at(row, 11)?,
                report_notes: row.get(22)?,
                external_approval_reference: None,
                analytes: Vec::new(),
            })
        })
        .optional()
}

pub(crate) fn analytes(
    conn: &Connection,
    id: MethodVersionId,
) -> rusqlite::Result<Vec<MethodAnalyteView>> {
    conn.prepare_cached(
        "SELECT a.code, a.name, x.analyte_id, x.unit, x.cutoff, x.limit_of_detection, \
         x.limit_of_quantitation, x.reportable_low, x.reportable_high, x.display_scale, x.reference_interval, x.range_policy \
         FROM cat_method_version_analytes x JOIN cat_analytes a ON a.id = x.analyte_id \
         WHERE x.method_version_id = ?1 ORDER BY a.code",
    )?
    .query_map([id], analyte_view)?
    .collect()
}

fn analyte_view(row: &Row<'_>) -> rusqlite::Result<MethodAnalyteView> {
    Ok(MethodAnalyteView {
        analyte_code: row.get(0)?,
        analyte_name: row.get(1)?,
        limits: MethodAnalyte {
            analyte_id: row.get(2)?,
            unit: row.get(3)?,
            cutoff: row.get(4)?,
            limit_of_detection: row.get(5)?,
            limit_of_quantitation: row.get(6)?,
            reportable_low: row.get(7)?,
            reportable_high: row.get(8)?,
            display_scale: row.get(9)?,
            reference_interval: row.get(10)?,
            range_policy: row.get(11)?,
        },
    })
}

/// Whether the method version lists `analyte`.
pub(crate) fn measures(
    conn: &Connection,
    id: MethodVersionId,
    analyte: AnalyteId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM cat_method_version_analytes \
         WHERE method_version_id = ?1 AND analyte_id = ?2)",
    )?
    .query_row(params![id, analyte], |row| row.get(0))
}

/// Analytes of method version `id` that orderable components expect, each
/// with the orderable's code and version number.
pub(crate) fn analytes_in_use(
    conn: &Connection,
    id: MethodVersionId,
) -> rusqlite::Result<Vec<(AnalyteId, String, i64)>> {
    conn.prepare_cached(
        "SELECT DISTINCT x.analyte_id, o.code, v.version FROM cat_orderable_component_analytes x \
         JOIN cat_orderable_versions v ON v.id = x.orderable_version_id \
         JOIN cat_orderables o ON o.id = v.orderable_id \
         WHERE x.method_version_id = ?1 ORDER BY o.code, v.version",
    )?
    .query_map([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
    .collect()
}

/// Analytes of method version `id` with no cutoff, with their codes.
pub(crate) fn blank_cutoffs(
    conn: &Connection,
    id: MethodVersionId,
) -> rusqlite::Result<Vec<(AnalyteId, String)>> {
    conn.prepare_cached(
        "SELECT x.analyte_id, a.code FROM cat_method_version_analytes x \
         JOIN cat_analytes a ON a.id = x.analyte_id \
         WHERE x.method_version_id = ?1 AND x.cutoff IS NULL ORDER BY a.code",
    )?
    .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect()
}

/// Current logical orderables whose current versions use `id`.
pub(crate) fn blocking_current_orderables(
    conn: &Connection,
    id: MethodVersionId,
) -> rusqlite::Result<Vec<OrderableId>> {
    conn.prepare_cached(
        "SELECT DISTINCT v.orderable_id FROM cat_orderable_components c \
         JOIN cat_orderable_versions v ON v.id = c.orderable_version_id \
         WHERE c.method_version_id = ?1 AND v.status = 'approved' AND v.effective_to IS NULL \
         ORDER BY v.orderable_id",
    )?
    .query_map([id], |row| row.get(0))?
    .collect()
}

/// Current approved orderable versions with a component whose method
/// version is not current, as (orderable code, version number).
pub(crate) fn current_orderables_on_replaced_methods(
    conn: &Connection,
) -> rusqlite::Result<Vec<(String, i64)>> {
    conn.prepare_cached(
        "SELECT DISTINCT o.code, ov.version FROM cat_orderable_versions ov \
         JOIN cat_orderables o ON o.id = ov.orderable_id \
         JOIN cat_orderable_components c ON c.orderable_version_id = ov.id \
         JOIN cat_method_versions mv ON mv.id = c.method_version_id \
         WHERE ov.status = 'approved' AND ov.effective_to IS NULL \
         AND NOT (mv.status = 'approved' AND mv.effective_to IS NULL) \
         ORDER BY o.code, ov.version",
    )?
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect()
}

/// Current logical orderables using the current version of `method_id`.
pub(crate) fn blocking_orderables_for_supersession(
    conn: &Connection,
    method_id: MethodId,
) -> rusqlite::Result<Vec<OrderableId>> {
    conn.prepare_cached(
        "SELECT DISTINCT ov.orderable_id FROM cat_method_versions mv \
         JOIN cat_orderable_components c ON c.method_version_id = mv.id \
         JOIN cat_orderable_versions ov ON ov.id = c.orderable_version_id \
         WHERE mv.method_id = ?1 AND mv.status = 'approved' AND mv.effective_to IS NULL \
         AND ov.status = 'approved' AND ov.effective_to IS NULL ORDER BY ov.orderable_id",
    )?
    .query_map([method_id], |row| row.get(0))?
    .collect()
}
