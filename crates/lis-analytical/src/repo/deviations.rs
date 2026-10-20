//! SQL for deviations.

use lis_catalog::AnalyteId;
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{DeviationId, DeviationScope};
use crate::views::DeviationView;

pub(crate) struct NewDeviation<'a> {
    pub(crate) id: DeviationId,
    pub(crate) scope: DeviationScope,
    pub(crate) scope_id: &'a str,
    pub(crate) analyte_id: Option<AnalyteId>,
    pub(crate) description: &'a str,
    pub(crate) reason: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn insert(conn: &Connection, new: &NewDeviation<'_>) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ana_deviations (id, scope, scope_id, analyte_id, description, reason, \
         state, opened_by, opened_at, revision, seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, \
         ?8, 1, (SELECT coalesce(max(seq), 0) + 1 FROM ana_deviations))",
    )?
    .execute(params![
        new.id,
        new.scope,
        new.scope_id,
        new.analyte_id,
        new.description,
        new.reason,
        new.by,
        new.at
    ])?;
    Ok(())
}

pub(crate) struct Closing<'a> {
    pub(crate) disposition: &'a str,
    pub(crate) reason: &'a str,
    pub(crate) by: &'a str,
    pub(crate) at: UtcTimestamp,
}

pub(crate) fn close(
    conn: &Connection,
    id: DeviationId,
    revision: i64,
    closing: &Closing<'_>,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ana_deviations SET state = 'closed', disposition = ?2, close_reason = ?3, \
         closed_by = ?4, closed_at = ?5, revision = ?6 WHERE id = ?1",
    )?
    .execute(params![
        id,
        closing.disposition,
        closing.reason,
        closing.by,
        closing.at,
        revision
    ])?;
    Ok(())
}

const COLUMNS: &str = "id, scope, scope_id, analyte_id, description, reason, state, \
     disposition, close_reason, opened_by, opened_at, closed_by, closed_at, revision";

fn from_row(row: &Row<'_>) -> rusqlite::Result<DeviationView> {
    Ok(DeviationView {
        deviation_id: row.get(0)?,
        scope: row.get(1)?,
        scope_id: row.get(2)?,
        analyte_id: row.get(3)?,
        description: row.get(4)?,
        reason: row.get(5)?,
        state: row.get(6)?,
        disposition: row.get(7)?,
        close_reason: row.get(8)?,
        opened_by: row.get(9)?,
        opened_at: row.get(10)?,
        closed_by: row.get(11)?,
        closed_at: row.get(12)?,
        revision: row.get(13)?,
    })
}

pub(crate) fn deviation(
    conn: &Connection,
    id: DeviationId,
) -> rusqlite::Result<Option<DeviationView>> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ana_deviations WHERE id = ?1"
    ))?
    .query_row([id], from_row)
    .optional()
}

/// Every deviation, open or closed, on one target, newest first.
pub(crate) fn on_target(
    conn: &Connection,
    scope: DeviationScope,
    scope_id: &str,
) -> rusqlite::Result<Vec<DeviationView>> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ana_deviations WHERE scope = ?1 AND scope_id = ?2 \
         ORDER BY opened_at DESC, id DESC"
    ))?
    .query_map(params![scope, scope_id], from_row)?
    .collect()
}

/// The deviations on one target that cover `analyte_id` (no analyte means
/// every analyte): the highest sequence among them, open or closed, and
/// whether any is open.
pub(crate) fn applicable(
    conn: &Connection,
    scope: DeviationScope,
    scope_id: &str,
    analyte_id: AnalyteId,
) -> rusqlite::Result<(Option<i64>, bool)> {
    conn.prepare_cached(
        "SELECT max(seq), coalesce(max(state = 'open'), 0) FROM ana_deviations \
         WHERE scope = ?1 AND scope_id = ?2 AND (analyte_id IS NULL OR analyte_id = ?3)",
    )?
    .query_row(params![scope, scope_id, analyte_id], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })
}

/// Open deviations, oldest first, at most `limit`.
pub(crate) fn open(conn: &Connection, limit: u32) -> rusqlite::Result<Vec<DeviationView>> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ana_deviations WHERE state = 'open' \
         ORDER BY opened_at, id LIMIT ?1"
    ))?
    .query_map([limit], from_row)?
    .collect()
}
