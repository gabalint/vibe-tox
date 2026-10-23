//! The runs screen's view of app runs (`list_app_runs`), with their listening gaps.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use ts_rs::TS;

use super::{AppRunId, Stamp, StopKind, is_listening_gap};
use crate::runtime_error::RuntimeError;

/// One run as the runs screen shows it (`list_app_runs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppRunView {
    /// The run.
    pub run_id: AppRunId,
    /// When it started.
    pub started_at: UtcTimestamp,
    /// When it stopped; `None` while open or after an unclean stop.
    pub stopped_at: Option<UtcTimestamp>,
    /// How it stopped.
    pub stop_kind: Option<StopKind>,
    /// The app version.
    pub app_version: String,
    /// The web engine.
    pub webview: Option<String>,
    /// Seconds since the previous run was last seen.
    #[ts(type = "number | null")]
    pub gap_seconds: Option<i64>,
    /// The previous run never stopped.
    pub previous_unclean: bool,
    /// Seconds of forward clock jumps while listeners were active, during this run.
    #[ts(type = "number | null")]
    pub clock_gap_seconds: Option<i64>,
    /// The run has a listening gap nobody has acknowledged yet.
    pub needs_acknowledgement: bool,
    /// Acknowledgement selected by its recorded timestamp, not an inferred causal order.
    pub acknowledged: Option<Stamp>,
}

const VIEW_COLUMNS: &str = "r.id, r.started_at, r.stopped_at, r.stop_kind, r.app_version, \
     r.webview_text, r.gap_seconds, r.previous_unclean, r.gap_acknowledged_by, \
     r.gap_acknowledged_at, \
     (SELECT sum(j.seconds) FROM ops_clock_jumps j WHERE j.run_id = r.id \
      AND j.direction = 'forward' AND j.listening = 1), \
     (SELECT count(*) FROM ops_clock_jumps j WHERE j.run_id = r.id \
      AND j.direction = 'forward' AND j.listening = 1 AND j.acknowledged_at IS NULL), \
     (SELECT j.acknowledged_by FROM ops_clock_jumps j WHERE j.run_id = r.id \
      AND j.acknowledged_at IS NOT NULL ORDER BY j.acknowledged_at DESC LIMIT 1), \
     (SELECT max(j.acknowledged_at) FROM ops_clock_jumps j WHERE j.run_id = r.id)";

fn run_view(row: &Row<'_>) -> rusqlite::Result<AppRunView> {
    let stamp = |by: Option<String>, at: Option<UtcTimestamp>| {
        by.zip(at).map(|(by, at)| Stamp {
            by,
            by_display: None,
            at,
        })
    };
    let start_ack = stamp(row.get(8)?, row.get(9)?);
    let clock_ack = stamp(row.get(12)?, row.get(13)?);
    let gap_seconds: Option<i64> = row.get(6)?;
    let open_clock_gaps: i64 = row.get(11)?;
    Ok(AppRunView {
        run_id: row.get(0)?,
        started_at: row.get(1)?,
        stopped_at: row.get(2)?,
        stop_kind: row.get(3)?,
        app_version: row.get(4)?,
        webview: row.get(5)?,
        gap_seconds,
        previous_unclean: row.get(7)?,
        clock_gap_seconds: row.get(10)?,
        needs_acknowledgement: (is_listening_gap(gap_seconds) && start_ack.is_none())
            || open_clock_gaps > 0,
        acknowledged: [start_ack, clock_ack]
            .into_iter()
            .flatten()
            .max_by(|a, b| a.at.cmp(&b.at)),
    })
}

pub fn get_app_run(conn: &Connection, run_id: AppRunId) -> Result<AppRunView, RuntimeError> {
    let sql = format!("SELECT {VIEW_COLUMNS} FROM ops_app_runs r WHERE r.id = ?1");
    conn.prepare_cached(&sql)?
        .query_row([run_id], run_view)
        .optional()?
        .ok_or(RuntimeError::NotFound("app run"))
}

/// App runs by recorded start time, newest first; timestamps are not causal ordering.
pub fn list_app_runs(conn: &Connection, limit: u32) -> Result<Vec<AppRunView>, RuntimeError> {
    list_app_runs_page(conn, limit, None)
}

/// A bounded history page. The immutable start timestamp plus ID breaks equal-time ties.
pub fn list_app_runs_page(
    conn: &Connection,
    limit: u32,
    before_run_id: Option<AppRunId>,
) -> Result<Vec<AppRunView>, RuntimeError> {
    if let Some(id) = before_run_id {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ops_app_runs WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(RuntimeError::NotFound("app run cursor"));
        }
    }
    let sql = format!(
        "SELECT {VIEW_COLUMNS} FROM ops_app_runs r \
         WHERE (?2 IS NULL OR (r.started_at, r.id) < \
           (SELECT started_at, id FROM ops_app_runs WHERE id = ?2)) \
         ORDER BY r.started_at DESC, r.id DESC LIMIT ?1"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_map(params![limit.clamp(1, 200), before_run_id], run_view)?
        .collect::<rusqlite::Result<_>>()?)
}

/// Every run with unresolved startup or listening-clock gaps, independent of history pages.
/// Queue callers apply their own stable task pagination after selecting pending work.
pub fn pending_listening_gaps(conn: &Connection) -> Result<Vec<AppRunView>, RuntimeError> {
    let sql = format!(
        "SELECT {VIEW_COLUMNS} FROM ops_app_runs r \
         WHERE (r.gap_seconds > ?1 AND r.gap_acknowledged_at IS NULL) \
         OR EXISTS(SELECT 1 FROM ops_clock_jumps j WHERE j.run_id = r.id \
             AND j.direction = 'forward' AND j.listening = 1 AND j.acknowledged_at IS NULL) \
         ORDER BY r.started_at, r.id"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_map([super::LISTENING_GAP_SECONDS], run_view)?
        .collect::<rusqlite::Result<_>>()?)
}
