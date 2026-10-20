//! Which targets are due for a copy (M4 design brief §3.1 step 1, §3.4).

use std::time::Duration;

use lis_audit::{ChainLink, EventFilter, Stream};
use lis_core::UtcTimestamp;
use rusqlite::Connection;

use super::{BACKUP, BackupError, repo};
use crate::policy::OperationsSettings;
use crate::targets::{self, Target};

/// Security events the backup and anchor jobs write about themselves. They change the
/// chain heads but no data worth a new copy; counting them would make every completion
/// event trigger the next copy. `ops.export_anchors` is the anchor job's action (M4 design
/// brief §2.4).
const BOOKKEEPING_ACTIONS: [&str; 2] = [BACKUP, crate::anchors::EXPORT_ANCHORS];
/// More bookkeeping events than this since a set count as a change.
const BOOKKEEPING_LIMIT: u32 = 1_000;

/// True when the job runner has backup work: a waiting manual or pre-update request, or
/// an available target that takes backups and is due. Reads target markers, so it runs
/// on a reader, never inside a write transaction.
pub fn is_due(
    conn: &Connection,
    now: UtcTimestamp,
    settings: &OperationsSettings,
) -> Result<bool, BackupError> {
    if repo::oldest_request(conn)?.is_some() {
        return Ok(true);
    }
    let available = available_backup_targets(targets::active(conn)?);
    Ok(!due_targets(conn, now, settings, available)?.is_empty())
}

/// The targets of `candidates` that take backups and are reachable now.
pub(super) fn available_backup_targets(candidates: Vec<Target>) -> Vec<Target> {
    candidates
        .into_iter()
        .filter(|target| target.purpose.takes_backups() && targets::is_available(target))
        .collect()
}

/// The `available` targets that are due: the interval passed since their last attempt,
/// and their newest verified set differs from the current chain heads.
pub(super) fn due_targets(
    conn: &Connection,
    now: UtcTimestamp,
    settings: &OperationsSettings,
    available: Vec<Target>,
) -> Result<Vec<Target>, BackupError> {
    let interval = Duration::from_secs(u64::from(settings.backup_interval_minutes) * 60);
    let current = lis_audit::heads(conn)?;
    let mut due = Vec::new();
    for target in available {
        let waited = repo::last_attempt(conn, target.id)?
            .is_none_or(|attempt| now.saturating_duration_since(attempt) >= interval);
        if waited && changed_since_newest_set(conn, &target, &current)? {
            due.push(target);
        }
    }
    Ok(due)
}

fn changed_since_newest_set(
    conn: &Connection,
    target: &Target,
    current: &[ChainLink],
) -> Result<bool, BackupError> {
    let Some(set_heads) = repo::newest_verified_heads(conn, target.id)? else {
        return Ok(true);
    };
    for head in current {
        let in_set = set_heads
            .iter()
            .find(|(stream, _, _)| stream == head.stream.as_str());
        let Some((_, seq, hash)) = in_set else {
            return Ok(true);
        };
        let same = head.seq == *seq && head.hash == *hash;
        let only_bookkeeping = head.stream == Stream::Security
            && head.seq > *seq
            && only_bookkeeping_since(conn, *seq, head.seq)?;
        if !same && !only_bookkeeping {
            return Ok(true);
        }
    }
    Ok(false)
}

/// True when every security event after `since` up to `head` is bookkeeping.
fn only_bookkeeping_since(conn: &Connection, since: i64, head: i64) -> Result<bool, BackupError> {
    let Ok(newer) = u32::try_from(head - since) else {
        return Ok(false);
    };
    if newer > BOOKKEEPING_LIMIT {
        return Ok(false);
    }
    let events = lis_audit::list(
        conn,
        &EventFilter {
            stream: Stream::Security,
            before_seq: None,
            limit: newer,
            action: None,
            record_type: None,
            record_id: None,
            self_approvals_only: false,
        },
    )?;
    Ok(events.len() == newer as usize
        && events
            .iter()
            .all(|event| event.seq > since && BOOKKEEPING_ACTIONS.contains(&event.action.as_str())))
}
