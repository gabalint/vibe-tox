//! SQL for report layout versions (`0006_report_layouts.sql`).

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::layouts::ReportLayout;
use crate::model::{LayoutVersionId, LayoutVersionState};

/// One layout version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayoutVersionRow {
    pub(crate) id: LayoutVersionId,
    pub(crate) version_no: i64,
    pub(crate) layout: ReportLayout,
    pub(crate) state: LayoutVersionState,
    pub(crate) drafted_by: UserId,
    pub(crate) drafted_at: UtcTimestamp,
    pub(crate) decided_by: Option<UserId>,
    pub(crate) decided_at: Option<UtcTimestamp>,
    pub(crate) rejection_reason: Option<String>,
    pub(crate) superseded_at: Option<UtcTimestamp>,
    pub(crate) revision: i64,
}

const COLUMNS: &str = "id, version_no, layout, state, drafted_by, drafted_at, decided_by, \
     decided_at, rejection_reason, superseded_at, revision FROM rpt_layout_versions";

fn version_row(row: &Row<'_>) -> rusqlite::Result<LayoutVersionRow> {
    Ok(LayoutVersionRow {
        id: row.get(0)?,
        version_no: row.get(1)?,
        layout: row.get(2)?,
        state: row.get(3)?,
        drafted_by: row.get(4)?,
        drafted_at: row.get(5)?,
        decided_by: row.get(6)?,
        decided_at: row.get(7)?,
        rejection_reason: row.get(8)?,
        superseded_at: row.get(9)?,
        revision: row.get(10)?,
    })
}

fn in_state(
    conn: &Connection,
    state: LayoutVersionState,
) -> rusqlite::Result<Option<LayoutVersionRow>> {
    conn.prepare_cached(&format!("SELECT {COLUMNS} WHERE state = ?1"))?
        .query_row([state], version_row)
        .optional()
}

/// The approved version in force, if any.
pub(crate) fn active(conn: &Connection) -> rusqlite::Result<Option<LayoutVersionRow>> {
    in_state(conn, LayoutVersionState::Active)
}

/// The draft awaiting a decision, if any.
pub(crate) fn draft(conn: &Connection) -> rusqlite::Result<Option<LayoutVersionRow>> {
    in_state(conn, LayoutVersionState::Draft)
}

pub(crate) fn get(
    conn: &Connection,
    id: LayoutVersionId,
) -> rusqlite::Result<Option<LayoutVersionRow>> {
    conn.prepare_cached(&format!("SELECT {COLUMNS} WHERE id = ?1"))?
        .query_row([id], version_row)
        .optional()
}

/// Every version, newest first.
pub(crate) fn all(conn: &Connection) -> rusqlite::Result<Vec<LayoutVersionRow>> {
    conn.prepare_cached(&format!("SELECT {COLUMNS} ORDER BY version_no DESC"))?
        .query_map([], version_row)?
        .collect()
}

pub(crate) fn next_version_no(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT coalesce(max(version_no), 0) + 1 FROM rpt_layout_versions",
        [],
        |row| row.get(0),
    )
}

pub(crate) fn insert(conn: &Connection, version: &LayoutVersionRow) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO rpt_layout_versions (id, version_no, layout, state, drafted_by, \
         drafted_at, revision) VALUES (?1, ?2, ?3, 'draft', ?4, ?5, 1)",
        params![
            version.id,
            version.version_no,
            version.layout,
            version.drafted_by,
            version.drafted_at
        ],
    )?;
    Ok(())
}

/// Decides a draft at `revision`: approved (`rejection_reason` absent) or
/// rejected. `false` when the draft moved on.
pub(crate) fn decide(
    conn: &Connection,
    id: LayoutVersionId,
    revision: i64,
    by: UserId,
    at: UtcTimestamp,
    rejection_reason: Option<&str>,
) -> rusqlite::Result<bool> {
    let state = if rejection_reason.is_some() {
        LayoutVersionState::Rejected
    } else {
        LayoutVersionState::Active
    };
    let changed = conn.execute(
        "UPDATE rpt_layout_versions SET state = ?1, decided_by = ?2, decided_at = ?3, \
         rejection_reason = ?4, revision = revision + 1 \
         WHERE id = ?5 AND state = 'draft' AND revision = ?6",
        params![state, by, at, rejection_reason, id, revision],
    )?;
    Ok(changed == 1)
}

/// Marks the active version superseded.
pub(crate) fn supersede(
    conn: &Connection,
    id: LayoutVersionId,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE rpt_layout_versions SET state = 'superseded', superseded_at = ?1, \
         revision = revision + 1 WHERE id = ?2 AND state = 'active'",
        params![at, id],
    )?;
    Ok(())
}
