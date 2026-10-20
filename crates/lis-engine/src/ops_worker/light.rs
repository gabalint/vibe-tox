//! One light-loop step (M4 design brief §6.1, §6.2): the run's heartbeat, wall-clock
//! jump detection, the keep-awake wish, and health evaluation every 5 minutes.

use std::sync::Arc;
use std::time::{Duration, Instant};

use lis_core::{CorrelationId, Principal, UtcTimestamp};
use lis_interfaces::ListenerState;
use lis_ops::RuntimeError;
use lis_ops::health::{ClockStep, classify_clock_step};

use super::{Operations, health, locked};

/// Health is evaluated, and its history recorded, this often.
const HEALTH_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// The system principal of heartbeats and clock-change events.
const COMPONENT: &str = "ops-runtime";

/// What the previous step saw.
#[derive(Debug, Default)]
pub(crate) struct LightState {
    /// The wall-clock and monotonic readings of the previous step.
    previous: Option<(UtcTimestamp, Instant)>,
    /// When health was last evaluated and recorded.
    evaluated: Option<Instant>,
}

/// One step; failures are logged and retried at the next step.
pub(super) async fn tick(ops: &Arc<Operations>) {
    if let Err(error) = heartbeat(ops).await {
        tracing::warn!(%error, "app run heartbeat failed");
    }
    if let Err(error) = check_clock(ops).await {
        tracing::warn!(%error, "clock check failed");
    }
    want_keep_awake(ops);
    let due = locked(&ops.light)
        .evaluated
        .is_none_or(|at| at.elapsed() >= HEALTH_INTERVAL);
    if due {
        match health::evaluate_and_record(ops, false).await {
            Ok(_) => locked(&ops.light).evaluated = Some(Instant::now()),
            Err(error) => tracing::warn!(%error, "health evaluation failed"),
        }
    }
}

async fn heartbeat(ops: &Operations) -> Result<(), RuntimeError> {
    let run_id = ops.run_id;
    let beat = ops
        .db
        .write(principal(), CorrelationId::generate(), move |ctx| {
            lis_ops::runs::heartbeat(ctx, run_id)
        })
        .await;
    match beat {
        // After "Quit anyway" the run is stopped; there is nothing left to measure.
        Err(RuntimeError::RunStopped) => Ok(()),
        other => other,
    }
}

/// Compares the wall clock's step with the monotonic clock's (NFR-26). A jump is recorded
/// and audited as `ops.clock_change` (M9): a backward jump raises the clock alert for 24
/// hours, read from the record so it survives a restart; a forward jump while listeners
/// are active is a listening gap to acknowledge.
async fn check_clock(ops: &Operations) -> Result<(), RuntimeError> {
    let settings = ops.db.read(lis_ops::settings::current).await?;
    let (now, instant) = (ops.clock.now(), Instant::now());
    let previous = locked(&ops.light).previous.replace((now, instant));
    let Some((wall_before, instant_before)) = previous else {
        return Ok(());
    };
    let step = classify_clock_step(
        wall_before,
        now,
        instant.saturating_duration_since(instant_before),
        settings.clock_jump_seconds,
    );
    if step == ClockStep::Steady {
        return Ok(());
    }
    let listening = listeners_active(ops);
    tracing::warn!(?step, listening, "wall clock jumped");
    let run_id = ops.run_id;
    ops.db
        .write(principal(), CorrelationId::generate(), move |ctx| {
            lis_ops::health::record_clock_jump(ctx, run_id, step, listening).map(drop)
        })
        .await
}

/// Holds the inhibitor while any listener is listening, connected or receiving; a
/// watched result folder reports as a listening listener (F13).
pub(super) fn want_keep_awake(ops: &Operations) {
    let active = listeners_active(ops);
    if let Some(keep_awake) = locked(&ops.keep_awake).as_ref() {
        keep_awake.want(active);
    }
}

/// True while any listener is listening, connected or receiving.
fn listeners_active(ops: &Operations) -> bool {
    ops.listeners.status_snapshot().iter().any(|status| {
        matches!(
            status.state,
            ListenerState::Listening | ListenerState::Connected | ListenerState::Receiving
        )
    })
}

fn principal() -> Principal {
    Principal::system(COMPONENT, [])
}
