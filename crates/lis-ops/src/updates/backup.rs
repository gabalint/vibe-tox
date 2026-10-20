//! Backup eligibility for update approvals (REQ-OPS-006).

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension};

use crate::RuntimeError;
use crate::backup::{RunId, RunKind, RunRecord, RunState};
use crate::policy::PRE_UPDATE_MAX_AGE_HOURS;

/// Whether a retained backup can be chosen for update approval (REQ-OPS-006).
/// Manual and pre-update backups must have an unpruned verified copy and have
/// finished within the last 24 hours. The command rechecks eligibility on save.
#[must_use]
pub fn update_approval_eligible(run: &RunRecord, now: UtcTimestamp) -> bool {
    matches!(run.kind, RunKind::Manual | RunKind::PreUpdate)
        && run.state == RunState::Succeeded
        && run.copies.iter().any(|copy| copy.succeeded && !copy.pruned)
        && run.finished_at.is_some_and(|at| backup_is_recent(at, now))
}

fn backup_is_recent(finished_at: UtcTimestamp, now: UtcTimestamp) -> bool {
    let max_age = std::time::Duration::from_secs(u64::from(PRE_UPDATE_MAX_AGE_HOURS) * 3600);
    finished_at <= now && now.saturating_duration_since(finished_at) <= max_age
}

/// Rechecks retained-copy eligibility and age inside the approval transaction.
/// The SQL view is also used by the store's insertion guard.
pub(super) fn ensure_pre_update_backup(
    conn: &Connection,
    run_id: RunId,
    now: UtcTimestamp,
) -> Result<(), RuntimeError> {
    let finished_at: Option<UtcTimestamp> = conn
        .prepare_cached("SELECT finished_at FROM ops_v_verified_pre_update_runs WHERE run_id = ?1")?
        .query_row([run_id], |row| row.get(0))
        .optional()?;
    match finished_at {
        Some(at) if backup_is_recent(at, now) => Ok(()),
        Some(_) | None => Err(RuntimeError::PreUpdateBackupInvalid),
    }
}
