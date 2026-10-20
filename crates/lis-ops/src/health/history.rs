//! Health history (REQ-OPS-008 maintenance record; M4 design brief D8): a row in
//! `ops_health_events` when a check's state differs from its last committed observation,
//! or its first actual evaluation establishes a missing current-state pointer.
//! Technical writes, not audited, like the outbox state.

use lis_core::{Id, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::checks::{CheckCode, HealthState};
use super::evaluate::CheckResult;
use crate::runtime_error::RuntimeError;

/// Marker for health event identifiers.
pub enum HealthEvent {}
/// Stable identifier of an immutable observed health transition.
pub type HealthEventId = Id<HealthEvent>;

/// One state change, as the history screen shows it (`list_health_history`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HealthEventView {
    /// Stable record identity, including repeated observations at equal timestamps.
    pub event_id: HealthEventId,
    /// The check.
    pub check: CheckCode,
    /// Its new state.
    pub state: HealthState,
    /// Why, as a machine code.
    pub detail_code: Option<String>,
    /// When the change was observed.
    pub observed_at: UtcTimestamp,
}

/// The last committed actual observation. Legacy history has no current event
/// until a real evaluation establishes its pointer; timestamps never choose it.
pub fn current_health_event(
    conn: &Connection,
    check: CheckCode,
) -> Result<Option<HealthEventView>, RuntimeError> {
    Ok(conn.prepare_cached(
        "SELECT e.id,e.check_code,e.state,e.detail_code,e.observed_at FROM ops_health_current c \
         JOIN ops_health_events e ON e.id=c.event_id WHERE c.check_code=?1",
    )?.query_row([check], event_from_row).optional()?)
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HealthEventView> {
    Ok(HealthEventView {
        event_id: row.get(0)?,
        check: row.get(1)?,
        state: row.get(2)?,
        detail_code: row.get(3)?,
        observed_at: row.get(4)?,
    })
}

fn last_state(conn: &Connection, check: CheckCode) -> Result<Option<HealthState>, RuntimeError> {
    Ok(current_health_event(conn, check)?.map(|event| event.state))
}

/// Appends a row for each result whose state differs from its check's last recorded
/// state (or that has none yet); returns how many were appended.
pub fn record_state_changes(
    ctx: &mut CommandContext<'_>,
    results: &[CheckResult],
) -> Result<usize, RuntimeError> {
    let mut appended = 0;
    for result in results {
        if last_state(ctx.tx(), result.check)? == Some(result.state) {
            continue;
        }
        let event_id = HealthEventId::generate();
        ctx.tx()
            .prepare_cached(
                "INSERT INTO ops_health_events (id, check_code, state, detail_code, value_text, \
                 observed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?
            .execute(params![
                event_id,
                result.check,
                result.state,
                result.detail_code,
                result.value,
                result.observed_at,
            ])?;
        ctx.tx().execute(
            "INSERT INTO ops_health_current(check_code,event_id) VALUES(?1,?2) \
             ON CONFLICT(check_code) DO UPDATE SET event_id = excluded.event_id",
            params![result.check, event_id],
        )?;
        appended += 1;
    }
    Ok(appended)
}

/// State changes, newest first, of one check or of all; up to one row beyond
/// `limit` so the caller can tell whether more exist (REQ-OPS-004).
pub fn list_health_history(
    conn: &Connection,
    check: Option<CheckCode>,
    limit: u32,
) -> Result<Vec<HealthEventView>, RuntimeError> {
    list_health_history_page(conn, check, limit, None)
}

/// Stable pages by recorded observation time and identity. This display order does
/// not establish causal ordering when the wall clock moves backward or ties.
pub fn list_health_history_page(
    conn: &Connection,
    check: Option<CheckCode>,
    limit: u32,
    before_event_id: Option<HealthEventId>,
) -> Result<Vec<HealthEventView>, RuntimeError> {
    let query_limit = limit.clamp(1, 500).saturating_add(1);
    Ok(conn
        .prepare_cached(
            "SELECT id, check_code, state, detail_code, observed_at FROM ops_health_events \
             WHERE (?1 IS NULL OR check_code = ?1) \
             AND (?3 IS NULL OR (observed_at, id) < \
                (SELECT observed_at, id FROM ops_health_events WHERE id = ?3)) \
             ORDER BY observed_at DESC, id DESC LIMIT ?2",
        )?
        .query_map(params![check, query_limit, before_event_id], event_from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_pages_preserve_equal_time_and_rolled_back_observations() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE ops_health_events(id TEXT PRIMARY KEY, check_code TEXT, state TEXT, detail_code TEXT, observed_at TEXT);").unwrap();
        let mut expected = Vec::new();
        for _ in 0..205 {
            let id = HealthEventId::generate();
            conn.execute(
                "INSERT INTO ops_health_events VALUES(?1,'backup_age','warning',NULL,?2)",
                params![id, "2026-10-14T12:00:00.000000000Z"],
            )
            .unwrap();
            expected.push(id);
        }
        expected.sort_by(|a, b| b.cmp(a));
        let rollback = HealthEventId::generate();
        conn.execute(
            "INSERT INTO ops_health_events VALUES(?1,'backup_age','ok',NULL,?2)",
            params![rollback, "2020-10-14T12:00:00.000000000Z"],
        )
        .unwrap();
        expected.push(rollback);
        let mut actual = Vec::new();
        let mut cursor = None;
        loop {
            let page =
                list_health_history_page(&conn, Some(CheckCode::BackupAge), 100, cursor).unwrap();
            let has_more = page.len() > 100;
            actual.extend(page.iter().take(100).map(|event| event.event_id));
            if !has_more {
                break;
            }
            cursor = actual.last().copied();
        }
        assert_eq!(actual, expected);
        assert!(
            list_health_history_page(&conn, Some(CheckCode::DiskFree), 100, None)
                .unwrap()
                .is_empty()
        );
    }
}
