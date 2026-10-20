//! Read models for backup targets, runs and status (REQ-OPS-007, NFR-04/30).

use std::time::Duration;

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{BackupError, CopyFailure, RunFailure, RunId, RunKind, RunState};
use crate::targets::{Target, TargetId, TargetKind, TargetPurpose};

/// A target with its retained lifecycle and newest copy facts.
#[derive(Debug, Clone)]
pub struct TargetRecord {
    pub target: Target,
    pub retired: bool,
    pub last_verified_at: Option<UtcTimestamp>,
    pub last_failure: Option<(CopyFailure, UtcTimestamp)>,
}

/// One target result within a backup run.
#[derive(Debug, Clone)]
pub struct CopyRecord {
    pub target_id: TargetId,
    pub target_label: String,
    pub set_id: Option<String>,
    pub succeeded: bool,
    pub failure: Option<CopyFailure>,
    pub verified_at: Option<UtcTimestamp>,
    pub pruned: bool,
}

/// One backup run and its target results.
#[derive(Debug, Clone)]
pub struct RunRecord {
    pub run_id: RunId,
    pub kind: RunKind,
    pub state: RunState,
    pub requested_by: Option<String>,
    pub requested_at: UtcTimestamp,
    pub started_at: Option<UtcTimestamp>,
    pub finished_at: Option<UtcTimestamp>,
    pub database_bytes: Option<u64>,
    pub artifact_count: Option<u64>,
    pub artifact_problems: u64,
    pub failure: Option<RunFailure>,
    pub copies: Vec<CopyRecord>,
}

/// Summary shown above the backup history.
#[derive(Debug, Clone)]
pub struct StatusRecord {
    pub next_due_at: Option<UtcTimestamp>,
    pub running: Option<RunRecord>,
    pub newest_verified: Option<(UtcTimestamp, String)>,
    pub up_to_date: bool,
}

/// Every target, including retired targets, in creation order.
pub fn list_targets(conn: &Connection) -> Result<Vec<TargetRecord>, BackupError> {
    let mut statement = conn.prepare_cached(
        "SELECT t.id, t.label, t.kind, t.purpose, t.path, t.revision, \
         t.retired_at IS NOT NULL, \
         (SELECT max(c.verified_at) FROM ops_backup_copies c \
          WHERE c.target_id = t.id AND c.state = 'succeeded'), \
         (SELECT c.failure_code FROM ops_backup_copies c JOIN ops_backup_runs r ON r.id = c.run_id \
          WHERE c.target_id = t.id AND c.state = 'failed' ORDER BY r.finished_at DESC LIMIT 1), \
         (SELECT r.finished_at FROM ops_backup_copies c JOIN ops_backup_runs r ON r.id = c.run_id \
          WHERE c.target_id = t.id AND c.state = 'failed' ORDER BY r.finished_at DESC LIMIT 1) \
         FROM ops_targets t ORDER BY t.rowid",
    )?;
    Ok(statement
        .query_map([], |row| {
            let failure: Option<CopyFailure> = row.get(8)?;
            let failed_at: Option<UtcTimestamp> = row.get(9)?;
            Ok(TargetRecord {
                target: Target {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    kind: row.get::<_, TargetKind>(2)?,
                    purpose: row.get::<_, TargetPurpose>(3)?,
                    path: row.get::<_, String>(4)?.into(),
                    revision: row.get(5)?,
                },
                retired: row.get(6)?,
                last_verified_at: row.get(7)?,
                last_failure: failure.zip(failed_at),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Backup runs newest first, optionally before one request time. Returns up
/// to one row beyond `limit` so the caller can show that more exist.
pub fn list_runs(
    conn: &Connection,
    limit: u32,
    before: Option<UtcTimestamp>,
) -> Result<Vec<RunRecord>, BackupError> {
    list_runs_page(conn, limit, before, None)
}

/// Timestamp-and-ID continuation includes equal request times; the anchor is immutable.
pub fn list_runs_page(
    conn: &Connection,
    limit: u32,
    before: Option<UtcTimestamp>,
    before_run_id: Option<RunId>,
) -> Result<Vec<RunRecord>, BackupError> {
    if let Some(id) = before_run_id {
        get_run(conn, id)?;
    }
    let query_limit = limit.min(500).saturating_add(1);
    let mut runs = conn
        .prepare_cached(
            "SELECT id, kind, state, requested_by, requested_at, started_at, finished_at, \
             snapshot_bytes, artifact_count, artifact_problems, failure_code \
             FROM ops_backup_runs WHERE (?1 IS NULL OR requested_at < ?1) \
             AND (?2 IS NULL OR (requested_at, id) < \
             (SELECT requested_at, id FROM ops_backup_runs WHERE id = ?2)) \
             ORDER BY requested_at DESC, id DESC LIMIT ?3",
        )?
        .query_map(params![before, before_run_id, query_limit], run_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for run in &mut runs {
        run.copies = copies(conn, run.run_id)?;
    }
    Ok(runs)
}

fn run_from_row(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    let unsigned = |index| -> rusqlite::Result<Option<u64>> {
        Ok(row
            .get::<_, Option<i64>>(index)?
            .and_then(|value| u64::try_from(value).ok()))
    };
    Ok(RunRecord {
        run_id: row.get(0)?,
        kind: row.get(1)?,
        state: row.get(2)?,
        requested_by: row.get(3)?,
        requested_at: row.get(4)?,
        started_at: row.get(5)?,
        finished_at: row.get(6)?,
        database_bytes: unsigned(7)?,
        artifact_count: unsigned(8)?,
        artifact_problems: unsigned(9)?.unwrap_or(0),
        failure: row.get(10)?,
        copies: Vec::new(),
    })
}

fn copies(conn: &Connection, run_id: RunId) -> Result<Vec<CopyRecord>, BackupError> {
    Ok(conn
        .prepare_cached(
            "SELECT c.target_id, t.label, c.set_id, c.state = 'succeeded', c.failure_code, \
             c.verified_at, c.pruned_at IS NOT NULL FROM ops_backup_copies c \
             JOIN ops_targets t ON t.id = c.target_id WHERE c.run_id = ?1 ORDER BY t.label, t.id",
        )?
        .query_map([run_id], |row| {
            Ok(CopyRecord {
                target_id: row.get(0)?,
                target_label: row.get(1)?,
                set_id: row.get(2)?,
                succeeded: row.get(3)?,
                failure: row.get(4)?,
                verified_at: row.get(5)?,
                pruned: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Current backup status. `available` is evaluated by the caller outside SQL and limits
/// which target IDs are considered when deciding whether a current copy exists.
pub fn status(
    conn: &Connection,
    now: UtcTimestamp,
    interval_minutes: u32,
    available: &[TargetId],
) -> Result<StatusRecord, BackupError> {
    let running_id: Option<RunId> = conn
        .prepare_cached(
            "SELECT id FROM ops_backup_runs WHERE state IN ('requested', 'running') \
             ORDER BY requested_at LIMIT 1",
        )?
        .query_row([], |row| row.get(0))
        .optional()?;
    let running = running_id.map(|id| get_run(conn, id)).transpose()?;
    let newest = newest_verified(conn, available)?;
    let current_heads: Vec<_> = lis_audit::heads(conn)?
        .into_iter()
        .map(|head| (head.stream.as_str().to_owned(), head.seq, head.hash))
        .collect();
    let up_to_date = newest.as_ref().is_some_and(|(run_id, _, _)| {
        run_heads(conn, *run_id).is_ok_and(|heads| heads == current_heads)
    });
    let next_due_at = newest.as_ref().and_then(|(_, at, _)| {
        at.checked_add(Duration::from_secs(u64::from(interval_minutes) * 60))
    });
    Ok(StatusRecord {
        next_due_at: next_due_at.filter(|due| *due > now),
        running,
        newest_verified: newest.map(|(_, at, label)| (at, label)),
        up_to_date,
    })
}

/// One backup run by ID.
pub fn get_run(conn: &Connection, id: RunId) -> Result<RunRecord, BackupError> {
    let mut run = conn
        .prepare_cached(
            "SELECT id, kind, state, requested_by, requested_at, started_at, finished_at, \
             snapshot_bytes, artifact_count, artifact_problems, failure_code \
             FROM ops_backup_runs WHERE id = ?1",
        )?
        .query_row([id], run_from_row)
        .optional()?
        .ok_or(BackupError::NotFound("backup run"))?;
    run.copies = copies(conn, id)?;
    Ok(run)
}

fn newest_verified(
    conn: &Connection,
    available: &[TargetId],
) -> Result<Option<(RunId, UtcTimestamp, String)>, BackupError> {
    let mut newest = None;
    for target in available {
        let candidate = conn
            .prepare_cached(
                "SELECT c.run_id, c.verified_at, t.label FROM ops_backup_copies c \
                 JOIN ops_targets t ON t.id = c.target_id \
                 WHERE c.target_id = ?1 AND c.state = 'succeeded' AND c.pruned_at IS NULL \
                 ORDER BY c.verified_at DESC LIMIT 1",
            )?
            .query_row([target], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .optional()?;
        if candidate
            .as_ref()
            .is_some_and(|(_, at, _)| newest.as_ref().is_none_or(|(_, old, _)| at > old))
        {
            newest = candidate;
        }
    }
    Ok(newest)
}

fn run_heads(conn: &Connection, run_id: RunId) -> Result<Vec<(String, i64, String)>, BackupError> {
    Ok(conn
        .prepare_cached(
            "SELECT stream, seq, hash FROM ops_backup_run_heads WHERE run_id = ?1 \
             ORDER BY CASE stream WHEN 'clinical' THEN 1 WHEN 'security' THEN 2 ELSE 3 END",
        )?
        .query_map([run_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
