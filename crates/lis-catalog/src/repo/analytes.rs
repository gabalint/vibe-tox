//! SQL for analyte corrections: the current row, its immutable history and
//! what refers to it.

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::commands::analytes::{AnalyteHistoryEntry, AnalyteMethodUse};
use crate::model::AnalyteId;
use crate::repo::lifecycle::IN_EFFECT;
use crate::views::AnalyteView;

/// One revision's facts, as stored in the row and in its history.
#[derive(Clone, Copy)]
pub(crate) struct AnalyteFacts<'a> {
    pub(crate) code: &'a str,
    pub(crate) name: &'a str,
    pub(crate) loinc_code: Option<&'a str>,
    pub(crate) drug_class_code: Option<&'a str>,
    pub(crate) active: bool,
}

/// Inserts a new, active analyte at revision 1.
pub(crate) fn insert(
    conn: &Connection,
    id: AnalyteId,
    facts: &AnalyteFacts<'_>,
    created_at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_analytes (id, code, name, loinc_code, drug_class_code, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        id,
        facts.code,
        facts.name,
        facts.loinc_code,
        facts.drug_class_code,
        created_at,
    ])?;
    Ok(())
}

/// `origin` is `registered` or `corrected`; a correction carries its reason.
pub(crate) struct HistoryRow<'a> {
    pub(crate) id: AnalyteId,
    pub(crate) revision: i64,
    pub(crate) facts: AnalyteFacts<'a>,
    pub(crate) recorded_at: UtcTimestamp,
    pub(crate) recorded_by: UserId,
    pub(crate) reason: Option<&'a str>,
    pub(crate) origin: &'static str,
}

pub(crate) fn insert_history(conn: &Connection, row: &HistoryRow<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_analyte_history (analyte_id, revision, code, name, loinc_code, \
         drug_class_code, active, recorded_at, recorded_by, reason, origin) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?
    .execute(params![
        row.id,
        row.revision,
        row.facts.code,
        row.facts.name,
        row.facts.loinc_code,
        row.facts.drug_class_code,
        row.facts.active,
        row.recorded_at,
        row.recorded_by,
        row.reason,
        row.origin,
    ])?;
    Ok(())
}

/// Moves the row to `revision`, whose history row must already exist (trigger).
pub(crate) fn update(
    conn: &Connection,
    id: AnalyteId,
    revision: i64,
    facts: &AnalyteFacts<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE cat_analytes SET code = ?2, name = ?3, loinc_code = ?4, drug_class_code = ?5, \
         active = ?6, revision = ?7 WHERE id = ?1",
    )?
    .execute(params![
        id,
        facts.code,
        facts.name,
        facts.loinc_code,
        facts.drug_class_code,
        facts.active,
        revision,
    ])?;
    Ok(())
}

pub(crate) fn get(conn: &Connection, id: AnalyteId) -> rusqlite::Result<Option<AnalyteView>> {
    conn.prepare_cached(
        "SELECT id, code, name, loinc_code, drug_class_code, active, revision \
         FROM cat_analytes WHERE id = ?1",
    )?
    .query_row([id], view)
    .optional()
}

pub(crate) fn view(row: &rusqlite::Row<'_>) -> rusqlite::Result<AnalyteView> {
    Ok(AnalyteView {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        loinc_code: row.get(3)?,
        drug_class_code: row.get(4)?,
        active: row.get(5)?,
        revision: row.get(6)?,
    })
}

pub(crate) fn history(
    conn: &Connection,
    id: AnalyteId,
) -> rusqlite::Result<Vec<AnalyteHistoryEntry>> {
    conn.prepare_cached(
        "SELECT revision, code, name, loinc_code, drug_class_code, active, recorded_at, \
         recorded_by, reason, origin FROM cat_analyte_history WHERE analyte_id = ?1 \
         ORDER BY revision",
    )?
    .query_map([id], |row| {
        Ok(AnalyteHistoryEntry {
            revision: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            loinc_code: row.get(3)?,
            drug_class_code: row.get(4)?,
            active: row.get(5)?,
            recorded_at: row.get(6)?,
            recorded_by: row.get(7)?,
            recorded_by_display: None,
            reason: row.get(8)?,
            origin: row.get(9)?,
        })
    })?
    .collect()
}

/// Whether any record refers to the analyte: method versions, source test-code
/// maps, billing class mappings, results, QC and report lines. The referring
/// columns are read from the schema's foreign keys, so a table added later is
/// covered without listing it here. Table and column names come from the
/// schema, never from input.
pub(crate) fn referenced(conn: &Connection, id: AnalyteId) -> rusqlite::Result<bool> {
    let columns: Vec<(String, String)> = conn
        .prepare_cached(
            "SELECT m.name, f.\"from\" FROM sqlite_schema m, pragma_foreign_key_list(m.name) f \
             WHERE m.type = 'table' AND f.\"table\" = 'cat_analytes' \
             AND m.name <> 'cat_analyte_history'",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (table, column) in columns {
        let sql = format!("SELECT EXISTS (SELECT 1 FROM \"{table}\" WHERE \"{column}\" = ?1)");
        if conn.prepare(&sql)?.query_row([id], |row| row.get(0))? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The method versions in effect or in draft that include the analyte.
pub(crate) fn current_methods(
    conn: &Connection,
    id: AnalyteId,
) -> rusqlite::Result<Vec<AnalyteMethodUse>> {
    let sql = format!(
        "SELECT m.code, m.name, v.version, v.status FROM cat_method_version_analytes x \
         JOIN cat_method_versions v ON v.id = x.method_version_id \
         JOIN cat_methods m ON m.id = v.method_id \
         WHERE x.analyte_id = ?1 AND (v.status = 'draft' OR ({IN_EFFECT})) \
         ORDER BY m.code, v.version"
    );
    conn.prepare_cached(&sql)?
        .query_map([id], |row| {
            Ok(AnalyteMethodUse {
                method_code: row.get(0)?,
                method_name: row.get(1)?,
                version: row.get(2)?,
                status: row.get(3)?,
            })
        })?
        .collect()
}
