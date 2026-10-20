//! Recorded wall-clock jumps (NFR-26; M4 review M9). The light loop records each jump
//! [`classify_clock_step`](super::classify_clock_step) reports, with whether analyzer
//! listeners were active, and audits it as `ops.clock_change`. Health reads the newest
//! backward jump from here, so its 24-hour alert survives a restart; a forward jump while
//! listening is a listening gap of the run, acknowledged with the run's other gaps
//! (`runs::acknowledge_gap`).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Id, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, params};

use super::ClockStep;
use crate::runs::AppRunId;
use crate::runtime_error::RuntimeError;

/// Audit action of a wall-clock jump (security stream).
pub const CLOCK_CHANGE: &str = "ops.clock_change";

/// Marker for clock-jump identifiers.
pub enum ClockJump {}
/// Identifies one recorded clock jump.
pub type ClockJumpId = Id<ClockJump>;

/// Records the jump `step` of run `run_id` and audits `ops.clock_change`; `listening`
/// says whether any analyzer listener was active. A steady step records nothing.
pub fn record_clock_jump(
    ctx: &mut CommandContext<'_>,
    run_id: AppRunId,
    step: ClockStep,
    listening: bool,
) -> Result<Option<ClockJumpId>, RuntimeError> {
    let (direction, seconds) = match step {
        ClockStep::Steady => return Ok(None),
        ClockStep::Backward { seconds } => ("backward", seconds),
        ClockStep::Forward { seconds } => ("forward", seconds),
    };
    let id = ClockJumpId::generate();
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_clock_jumps (id, run_id, detected_at, direction, seconds, \
             listening) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?
        .execute(params![
            id,
            run_id,
            ctx.now(),
            direction,
            i64::try_from(seconds).unwrap_or(i64::MAX),
            listening
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, CLOCK_CHANGE, Outcome::Succeeded)
            .record("app_run", run_id)
            .detail("clock_jump_id", AuditDetail::id(id))
            .detail("direction", AuditDetail::code(direction))
            .detail("seconds", AuditDetail::unsigned(seconds))
            .detail("listening", AuditDetail::boolean(listening)),
    )?;
    Ok(Some(id))
}

/// When the newest backward jump was detected, if any (the clock check's input).
pub fn last_backward_jump(conn: &Connection) -> Result<Option<UtcTimestamp>, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT max(detected_at) FROM ops_clock_jumps WHERE direction = 'backward'",
        )?
        .query_row([], |row| row.get(0))?)
}

/// Forward jumps while listening that nobody has acknowledged, in every run.
pub(crate) fn unacknowledged_gaps(conn: &Connection) -> Result<u32, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT count(*) FROM ops_clock_jumps WHERE direction = 'forward' \
             AND listening = 1 AND acknowledged_at IS NULL",
        )?
        .query_row([], |row| row.get(0))?)
}

/// Acknowledges run `run_id`'s open clock gaps as `by` at `at`; returns their seconds,
/// or `None` when it had none.
pub(crate) fn acknowledge_run_gaps(
    ctx: &CommandContext<'_>,
    run_id: AppRunId,
    by: &str,
) -> Result<Option<i64>, RuntimeError> {
    let seconds: Option<i64> = ctx
        .tx()
        .prepare_cached(
            "SELECT sum(seconds) FROM ops_clock_jumps WHERE run_id = ?1 \
             AND direction = 'forward' AND listening = 1 AND acknowledged_at IS NULL",
        )?
        .query_row([run_id], |row| row.get(0))?;
    if seconds.is_some() {
        ctx.tx()
            .prepare_cached(
                "UPDATE ops_clock_jumps SET acknowledged_by = ?2, acknowledged_at = ?3 \
                 WHERE run_id = ?1 AND direction = 'forward' AND listening = 1 \
                 AND acknowledged_at IS NULL",
            )?
            .execute(params![run_id, by, ctx.now()])?;
    }
    Ok(seconds)
}
