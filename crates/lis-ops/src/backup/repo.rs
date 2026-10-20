//! SQL for `ops_backup_runs`, `ops_backup_run_heads` and `ops_backup_copies`.

use lis_audit::ChainLink;
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::{CopyFailure, RunFailure, RunId, RunKind, RunState};
use crate::targets::TargetId;

/// A run waiting for the job runner.
pub(super) struct Request {
    pub(super) id: RunId,
    pub(super) kind: RunKind,
    pub(super) requested_by: String,
}

/// The oldest manual or pre-update request still waiting.
pub(super) fn oldest_request(conn: &Connection) -> rusqlite::Result<Option<Request>> {
    conn.prepare_cached(
        "SELECT id, kind, requested_by FROM ops_backup_runs WHERE state = 'requested' \
         ORDER BY requested_at, id LIMIT 1",
    )?
    .query_row([], |row| {
        Ok(Request {
            id: row.get(0)?,
            kind: row.get(1)?,
            requested_by: row.get(2)?,
        })
    })
    .optional()
}

pub(super) fn insert_request(
    conn: &Connection,
    id: RunId,
    kind: RunKind,
    requested_by: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ops_backup_runs (id, kind, state, requested_by, requested_at, revision) \
         VALUES (?1, ?2, 'requested', ?3, ?4, 1)",
    )?
    .execute(params![id, kind, requested_by, now])?;
    Ok(())
}

pub(super) fn insert_scheduled(
    conn: &Connection,
    id: RunId,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ops_backup_runs (id, kind, state, requested_at, started_at, revision) \
         VALUES (?1, 'scheduled', 'running', ?2, ?2, 1)",
    )?
    .execute(params![id, now])?;
    Ok(())
}

/// Moves a requested run to `running`; returns its new revision.
pub(super) fn start_requested(
    conn: &Connection,
    id: RunId,
    now: UtcTimestamp,
) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "UPDATE ops_backup_runs SET state = 'running', started_at = ?2, revision = revision + 1 \
         WHERE id = ?1 RETURNING revision",
    )?
    .query_row(params![id, now], |row| row.get(0))
}

/// The snapshot facts of a finished run.
pub(super) struct SnapshotRecord<'a> {
    pub(super) sha256: &'a str,
    pub(super) bytes: u64,
    pub(super) artifact_count: usize,
    pub(super) artifact_problems: usize,
}

/// Moves a running run to `succeeded` or `failed`; returns its new revision.
pub(super) fn finish(
    conn: &Connection,
    id: RunId,
    state: RunState,
    failure: Option<RunFailure>,
    snapshot: Option<&SnapshotRecord<'_>>,
    now: UtcTimestamp,
) -> rusqlite::Result<i64> {
    let count = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
    conn.prepare_cached(
        "UPDATE ops_backup_runs SET state = ?2, failure_code = ?3, finished_at = ?4, \
         snapshot_sha256 = ?5, snapshot_bytes = ?6, artifact_count = ?7, \
         artifact_problems = ?8, revision = revision + 1 WHERE id = ?1 RETURNING revision",
    )?
    .query_row(
        params![
            id,
            state,
            failure,
            now,
            snapshot.map(|s| s.sha256),
            snapshot.map(|s| i64::try_from(s.bytes).unwrap_or(i64::MAX)),
            snapshot.map(|s| count(s.artifact_count)),
            snapshot.map_or(0, |s| count(s.artifact_problems)),
        ],
        |row| row.get(0),
    )
}

/// Moves a requested or running run to `interrupted`.
pub(super) fn interrupt(conn: &Connection, id: RunId, now: UtcTimestamp) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ops_backup_runs SET state = 'interrupted', finished_at = ?2, \
         revision = revision + 1 WHERE id = ?1",
    )?
    .execute(params![id, now])?;
    Ok(())
}

/// Runs still `requested` or `running`, with their revisions.
pub(super) fn unfinished(conn: &Connection) -> rusqlite::Result<Vec<(RunId, i64)>> {
    conn.prepare_cached(
        "SELECT id, revision FROM ops_backup_runs WHERE state IN ('requested', 'running') \
         ORDER BY requested_at, id",
    )?
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect()
}

pub(super) fn insert_heads(
    conn: &Connection,
    id: RunId,
    heads: &[ChainLink],
) -> rusqlite::Result<()> {
    let mut insert = conn.prepare_cached(
        "INSERT INTO ops_backup_run_heads (run_id, stream, seq, hash) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for head in heads {
        insert.execute(params![id, head.stream.as_str(), head.seq, head.hash])?;
    }
    Ok(())
}

/// One copy row: verified with its set, or failed with its code.
pub(super) struct CopyRecord<'a> {
    pub(super) target_id: TargetId,
    pub(super) set_id: Option<&'a str>,
    pub(super) failure: Option<CopyFailure>,
    pub(super) written_bytes: u64,
    pub(super) verified_at: Option<UtcTimestamp>,
}

pub(super) fn insert_copy(
    conn: &Connection,
    run_id: RunId,
    copy: &CopyRecord<'_>,
) -> rusqlite::Result<()> {
    let state = if copy.failure.is_some() {
        "failed"
    } else {
        "succeeded"
    };
    conn.prepare_cached(
        "INSERT INTO ops_backup_copies (run_id, target_id, set_id, state, failure_code, \
         written_bytes, verified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        run_id,
        copy.target_id,
        copy.set_id,
        state,
        copy.failure,
        i64::try_from(copy.written_bytes).unwrap_or(i64::MAX),
        copy.verified_at,
    ])?;
    Ok(())
}

/// Marks the target's verified copy of `set_id` pruned, if one is recorded.
pub(super) fn mark_pruned(
    conn: &Connection,
    target_id: TargetId,
    set_id: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ops_backup_copies SET pruned_at = ?3 \
         WHERE target_id = ?1 AND set_id = ?2 AND state = 'succeeded' AND pruned_at IS NULL",
    )?
    .execute(params![target_id, set_id, now])?;
    Ok(())
}

/// Whether this exact published set has a successful, still-present copy row.
pub(super) fn successful_copy_exists(
    conn: &Connection,
    target_id: TargetId,
    run_id: RunId,
    set_id: &str,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT 1 FROM ops_backup_copies WHERE target_id = ?1 AND run_id = ?2 \
         AND set_id = ?3 AND state = 'succeeded' AND pruned_at IS NULL",
    )?
    .exists(params![target_id, run_id, set_id])
}

/// When a run last tried to copy to the target. A scheduled run that failed before any
/// copy (no key, too little space, a failed snapshot) counts as an attempt on every
/// target, so the job runner waits for the interval instead of starting another failing
/// run at once.
pub(super) fn last_attempt(
    conn: &Connection,
    target_id: TargetId,
) -> rusqlite::Result<Option<UtcTimestamp>> {
    conn.prepare_cached(
        "SELECT max(r.started_at) FROM ops_backup_runs r \
         WHERE EXISTS (SELECT 1 FROM ops_backup_copies c \
                       WHERE c.run_id = r.id AND c.target_id = ?1) \
            OR (r.kind = 'scheduled' AND r.state = 'failed' \
                AND NOT EXISTS (SELECT 1 FROM ops_backup_copies c WHERE c.run_id = r.id))",
    )?
    .query_row([target_id], |row| row.get(0))
}

/// The chain heads, as `(stream, seq, hash)`, of the target's newest verified set.
pub(super) fn newest_verified_heads(
    conn: &Connection,
    target_id: TargetId,
) -> rusqlite::Result<Option<Vec<(String, i64, String)>>> {
    let run: Option<RunId> = conn
        .prepare_cached(
            "SELECT c.run_id FROM ops_backup_copies c JOIN ops_backup_runs r ON r.id = c.run_id \
             WHERE c.target_id = ?1 AND c.state = 'succeeded' \
             ORDER BY r.started_at DESC, r.id DESC LIMIT 1",
        )?
        .query_row([target_id], |row| row.get(0))
        .optional()?;
    let Some(run) = run else {
        return Ok(None);
    };
    let heads = conn
        .prepare_cached("SELECT stream, seq, hash FROM ops_backup_run_heads WHERE run_id = ?1")?
        .query_map([run], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(heads))
}
