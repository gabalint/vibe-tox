//! SQL for `ops_exit_exports`, and the list query.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::package::Written;
use super::{ExitExportId, ExitExportReport, ExportFailure, ExportState};
use crate::backup::BackupError;

pub(super) fn any_unfinished(conn: &Connection) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT 1 FROM ops_exit_exports WHERE state IN ('requested', 'running')")?
        .exists([])
}

pub(super) fn insert_request(
    conn: &Connection,
    id: ExitExportId,
    requested_by: &str,
    folder_label: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ops_exit_exports (id, state, requested_by, requested_at, folder_label, \
         revision) VALUES (?1, 'requested', ?2, ?3, ?4, 1)",
    )?
    .execute(params![id, requested_by, now, folder_label])?;
    Ok(())
}

/// Moves a requested export to `running`; returns its new revision, `None` when it is not
/// waiting.
pub(super) fn start(conn: &Connection, id: ExitExportId) -> rusqlite::Result<Option<i64>> {
    conn.prepare_cached(
        "UPDATE ops_exit_exports SET state = 'running', revision = revision + 1 \
         WHERE id = ?1 AND state = 'requested' RETURNING revision",
    )?
    .query_row([id], |row| row.get(0))
    .optional()
}

/// Records the end of a running export; returns its new revision.
pub(super) fn finish(
    conn: &Connection,
    report: &ExitExportReport,
    written: Option<&Written>,
    now: UtcTimestamp,
) -> rusqlite::Result<i64> {
    let count = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
    conn.prepare_cached(
        "UPDATE ops_exit_exports SET state = ?2, failure_code = ?3, finished_at = ?4, \
         package_name = ?5, manifest_sha256 = ?6, database_bytes = ?7, artifact_count = ?8, \
         artifact_problems = ?9, revision = revision + 1 WHERE id = ?1 RETURNING revision",
    )?
    .query_row(
        params![
            report.export_id,
            report.state,
            report.failure,
            now,
            report.package_name,
            report.manifest_sha256,
            written.map(|written| i64::try_from(written.database_bytes).unwrap_or(i64::MAX)),
            written.map(|written| count(written.artifact_count)),
            written.map(|written| count(written.artifact_problems)),
        ],
        |row| row.get(0),
    )
}

/// Exports still `requested` or `running`, with their revisions.
pub(crate) fn unfinished(conn: &Connection) -> rusqlite::Result<Vec<(ExitExportId, i64)>> {
    conn.prepare_cached(
        "SELECT id, revision FROM ops_exit_exports WHERE state IN ('requested', 'running') \
         ORDER BY requested_at, id",
    )?
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect()
}

/// Moves a requested or running export to `interrupted`.
pub(crate) fn interrupt(
    conn: &Connection,
    id: ExitExportId,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ops_exit_exports SET state = 'interrupted', finished_at = ?2, \
         revision = revision + 1 WHERE id = ?1",
    )?
    .execute(params![id, now])?;
    Ok(())
}

/// An exit export as the export screen lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitExportRecord {
    /// The export.
    pub export_id: ExitExportId,
    /// Where it stands.
    pub state: ExportState,
    /// Who asked for it (stored actor text).
    pub requested_by: String,
    /// When.
    pub requested_at: UtcTimestamp,
    /// When it ended.
    pub finished_at: Option<UtcTimestamp>,
    /// The chosen folder's last name.
    pub folder_label: String,
    /// The package folder's name, once written.
    pub package_name: Option<String>,
    /// Length of the database copy.
    pub database_bytes: Option<u64>,
    /// Artifacts copied.
    pub artifact_count: Option<u64>,
    /// Artifacts the package lacks; listed in its manifest.
    pub artifact_problems: Option<u64>,
    /// SHA-256 of `manifest.json`.
    pub manifest_sha256: Option<String>,
    /// Why it failed.
    pub failure: Option<ExportFailure>,
}

/// Exit exports newest first, with one row beyond `limit` when it exists.
pub fn list_exit_exports(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<ExitExportRecord>, BackupError> {
    let query_limit = limit.min(500).saturating_add(1);
    Ok(conn
        .prepare_cached(
            "SELECT id, state, requested_by, requested_at, finished_at, folder_label, \
             package_name, database_bytes, artifact_count, artifact_problems, manifest_sha256, \
             failure_code FROM ops_exit_exports ORDER BY requested_at DESC, id DESC LIMIT ?1",
        )?
        .query_map([query_limit], record_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn record_from_row(row: &Row<'_>) -> rusqlite::Result<ExitExportRecord> {
    let unsigned = |index: usize| -> rusqlite::Result<Option<u64>> {
        Ok(row
            .get::<_, Option<i64>>(index)?
            .map(|value| u64::try_from(value).unwrap_or(0)))
    };
    Ok(ExitExportRecord {
        export_id: row.get(0)?,
        state: row.get(1)?,
        requested_by: row.get(2)?,
        requested_at: row.get(3)?,
        finished_at: row.get(4)?,
        folder_label: row.get(5)?,
        package_name: row.get(6)?,
        database_bytes: unsigned(7)?,
        artifact_count: unsigned(8)?,
        artifact_problems: unsigned(9)?,
        manifest_sha256: row.get(10)?,
        failure: row.get(11)?,
    })
}
