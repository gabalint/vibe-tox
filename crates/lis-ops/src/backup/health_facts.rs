//! The backup facts health evaluates (M4 design brief §3.5): backup, off-host and offline
//! copy ages, the restore-test age, the recovery kit, artifact problems and snapshot
//! growth, failure streaks and the newest snapshot's integrity, read from the backup
//! area's records for the engine (REQ-OPS-007, REQ-OPS-008). The backup age is the age of
//! the oldest audit event newer than the newest verified set's chain heads, so 0 means up
//! to date; the backup and anchor jobs' own completion events do not count, as in
//! [`is_due`](super::is_due). Streaks are ordered by run start, then run ID.

use std::time::Duration;

use lis_audit::{EventFilter, Stream};
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::{BACKUP, BackupError, RunFailure, RunId};
use crate::format::KeyId;
use crate::health::{BackupFacts, KitFacts};
use crate::targets::{self, Target, TargetKind};
use crate::{kit, restore};

/// Security events the backup and anchor jobs write about themselves.
const BOOKKEEPING_ACTIONS: [&str; 2] = [BACKUP, crate::anchors::EXPORT_ANCHORS];
/// More events than this after a set are not inspected one by one.
const BOOKKEEPING_LIMIT: i64 = 1_000;
/// Restore tests inspected for the newest pass.
const RESTORE_TESTS: u32 = 200;
/// Snapshot growth is averaged over this many days.
const GROWTH_DAYS: i64 = 7;
const DAY_SECONDS: u64 = 24 * 60 * 60;

#[cfg(test)]
#[path = "health_facts_tests.rs"]
mod tests;

/// The newest verified set of a target: its run, when it was verified and its chain
/// heads.
struct NewestSet {
    run_id: RunId,
    started_at: UtcTimestamp,
    verified_at: UtcTimestamp,
    heads: Vec<(String, i64)>,
}

/// Backup facts at `now`. `key_id` is `None` while the installation key is not usable;
/// the recovery kit then counts as outdated.
pub fn health_facts(
    conn: &Connection,
    now: UtcTimestamp,
    key_id: Option<KeyId>,
) -> Result<BackupFacts, BackupError> {
    let targets: Vec<Target> = targets::active(conn)?
        .into_iter()
        .filter(|target| target.purpose.takes_backups())
        .collect();
    let (mut backup_age, mut offhost_age, mut offline_age) = (None, None, None);
    let mut failed_copies_in_a_row = 0;
    for target in &targets {
        let set = newest_set(conn, target)?;
        failed_copies_in_a_row =
            failed_copies_in_a_row.max(failed_copies_since(conn, target, set.as_ref())?);
        let Some(set) = set else {
            continue;
        };
        let age = set_age(conn, &set, now)?;
        backup_age = youngest(backup_age, age);
        if target.kind.is_off_host() {
            offhost_age = youngest(offhost_age, age);
        }
        if target.kind == TargetKind::Removable && !targets::is_available(target) {
            offline_age = youngest(offline_age, now.saturating_duration_since(set.verified_at));
        }
    }
    let count = |kind: Option<TargetKind>| {
        let matching = targets
            .iter()
            .filter(|target| kind.is_none_or(|kind| target.kind == kind))
            .count();
        u32::try_from(matching).unwrap_or(u32::MAX)
    };
    Ok(BackupFacts {
        active_targets: count(None),
        removable_targets: count(Some(TargetKind::Removable)),
        backup_age,
        offhost_age,
        offline_age,
        last_passed_restore_test_at: restore::list_restore_tests(conn, RESTORE_TESTS)?
            .into_iter()
            .filter(restore::RestoreTestRecord::passed)
            .filter_map(|test| test.finished_at)
            .max(),
        kit: kit_facts(conn, key_id)?,
        artifact_problems: newest_artifact_problems(conn)?,
        failed_runs_in_a_row: failed_runs_in_a_row(conn)?,
        failed_copies_in_a_row,
        snapshot_intact: newest_snapshot_intact(conn)?,
    })
}

/// The average growth of succeeded snapshots per day over the last 7 days, when two
/// snapshots at least a day apart exist.
pub fn snapshot_growth_per_day(
    conn: &Connection,
    now: UtcTimestamp,
) -> Result<Option<u64>, BackupError> {
    let rows: Vec<(UtcTimestamp, i64)> = conn
        .prepare_cached(
            "SELECT finished_at, snapshot_bytes FROM ops_backup_runs \
             WHERE state = 'succeeded' AND julianday(?1) - julianday(finished_at) <= ?2 \
             ORDER BY finished_at",
        )?
        .query_map(params![now, GROWTH_DAYS], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
        return Ok(None);
    };
    let seconds = last.0.saturating_duration_since(first.0).as_secs();
    if seconds < DAY_SECONDS {
        return Ok(None);
    }
    let grown = u64::try_from(last.1.saturating_sub(first.1)).unwrap_or(0);
    Ok(Some(grown.saturating_mul(DAY_SECONDS) / seconds))
}

fn youngest(current: Option<Duration>, age: Duration) -> Option<Duration> {
    Some(current.map_or(age, |current| current.min(age)))
}

fn newest_set(conn: &Connection, target: &Target) -> Result<Option<NewestSet>, BackupError> {
    let newest: Option<(RunId, UtcTimestamp, UtcTimestamp)> = conn
        .prepare_cached(
            "SELECT c.run_id, r.started_at, c.verified_at FROM ops_backup_copies c \
             JOIN ops_backup_runs r ON r.id = c.run_id \
             WHERE c.target_id = ?1 AND c.state = 'succeeded' \
             ORDER BY r.started_at DESC, r.id DESC LIMIT 1",
        )?
        .query_row([target.id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?;
    let Some((run_id, started_at, verified_at)) = newest else {
        return Ok(None);
    };
    let heads = conn
        .prepare_cached("SELECT stream, seq FROM ops_backup_run_heads WHERE run_id = ?1")?
        .query_map([run_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(NewestSet {
        run_id,
        started_at,
        verified_at,
        heads,
    }))
}

/// The age of the oldest event the set lacks, ignoring bookkeeping; zero when none.
fn set_age(conn: &Connection, set: &NewestSet, now: UtcTimestamp) -> Result<Duration, BackupError> {
    let mut oldest: Option<UtcTimestamp> = None;
    for head in lis_audit::heads(conn)? {
        let in_set = set
            .heads
            .iter()
            .find(|(stream, _)| stream == head.stream.as_str())
            .map_or(0, |(_, seq)| *seq);
        let Some(first_missing) = first_counted_event(conn, head.stream, in_set, head.seq)? else {
            continue;
        };
        if let Some(at) = lis_audit::event_recorded_at(conn, head.stream, first_missing)? {
            oldest = Some(oldest.map_or(at, |oldest| oldest.min(at)));
        }
    }
    Ok(oldest.map_or(Duration::ZERO, |at| now.saturating_duration_since(at)))
}

/// The first event after `since` up to `head` that counts as a change, if any.
fn first_counted_event(
    conn: &Connection,
    stream: Stream,
    since: i64,
    head: i64,
) -> Result<Option<i64>, BackupError> {
    if head <= since {
        return Ok(None);
    }
    if stream != Stream::Security || head - since > BOOKKEEPING_LIMIT {
        return Ok(Some(since + 1));
    }
    let events = lis_audit::list(
        conn,
        &EventFilter {
            stream,
            before_seq: None,
            limit: u32::try_from(head - since).unwrap_or(u32::MAX),
            action: None,
            record_type: None,
            record_id: None,
            self_approvals_only: false,
        },
    )?;
    Ok(events
        .iter()
        .filter(|event| event.seq > since && !BOOKKEEPING_ACTIONS.contains(&event.action.as_str()))
        .map(|event| event.seq)
        .min())
}

fn kit_facts(conn: &Connection, key_id: Option<KeyId>) -> Result<KitFacts, BackupError> {
    let Some(key_id) = key_id else {
        return Ok(KitFacts {
            last_export_at: None,
            outdated: true,
            last_confirmation_at: None,
        });
    };
    let status = kit::status(conn, key_id)?;
    Ok(KitFacts {
        last_export_at: status.latest.as_ref().map(|kit| kit.exported_at),
        outdated: !status.outdated_reasons.is_empty(),
        last_confirmation_at: status
            .last_confirmation
            .map(|confirmation| confirmation.confirmed_at),
    })
}

/// Missing or damaged artifacts found by the newest finished backup run.
fn newest_artifact_problems(conn: &Connection) -> Result<u32, BackupError> {
    Ok(conn
        .prepare_cached(
            "SELECT artifact_problems FROM ops_backup_runs \
             WHERE state IN ('succeeded', 'failed') AND finished_at IS NOT NULL \
             ORDER BY finished_at DESC, id DESC LIMIT 1",
        )?
        .query_row([], |row| row.get(0))
        .optional()?
        .unwrap_or(0))
}

/// Failed runs since the newest succeeded run; interrupted runs do not count (§3.5).
fn failed_runs_in_a_row(conn: &Connection) -> Result<u32, BackupError> {
    let newest: Option<(UtcTimestamp, RunId)> = conn
        .prepare_cached(
            "SELECT started_at, id FROM ops_backup_runs WHERE state = 'succeeded' \
             ORDER BY started_at DESC, id DESC LIMIT 1",
        )?
        .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let (started_at, run_id) = newest.unzip();
    Ok(conn
        .prepare_cached(
            "SELECT COUNT(*) FROM ops_backup_runs WHERE state = 'failed' \
             AND (?1 IS NULL OR (started_at, id) > (?1, ?2))",
        )?
        .query_row(params![started_at, run_id], |row| row.get(0))?)
}

/// Failed copies to `target` since its newest verified set (§3.5).
fn failed_copies_since(
    conn: &Connection,
    target: &Target,
    newest: Option<&NewestSet>,
) -> Result<u32, BackupError> {
    let (started_at, run_id) = newest.map(|set| (set.started_at, set.run_id)).unzip();
    Ok(conn
        .prepare_cached(
            "SELECT COUNT(*) FROM ops_backup_copies c JOIN ops_backup_runs r ON r.id = c.run_id \
             WHERE c.target_id = ?1 AND c.state = 'failed' \
             AND (?2 IS NULL OR (r.started_at, r.id) > (?2, ?3))",
        )?
        .query_row(params![target.id, started_at, run_id], |row| row.get(0))?)
}

/// Whether the newest snapshot that reached its integrity check passed it (§3.1 step 4):
/// an intact snapshot is recorded with its digest; `None` before the first check.
fn newest_snapshot_intact(conn: &Connection) -> Result<Option<bool>, BackupError> {
    Ok(conn
        .prepare_cached(
            "SELECT snapshot_sha256 IS NOT NULL FROM ops_backup_runs \
             WHERE finished_at IS NOT NULL \
             AND (snapshot_sha256 IS NOT NULL OR failure_code = ?1) \
             ORDER BY finished_at DESC, id DESC LIMIT 1",
        )?
        .query_row([RunFailure::SnapshotNotIntact], |row| row.get(0))
        .optional()?)
}
