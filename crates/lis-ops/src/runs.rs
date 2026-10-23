//! App runs (REQ-OPS-013, REQ-OPS-002; M4 design brief §2.3, §6.4): one row per start of
//! the app, `open` until stopped `clean` or `quit_with_warnings`, once. A run still open
//! when the next run starts stopped uncleanly; the new run records that, the gap since the
//! old run was last seen, and any change of app version or web engine. A gap longer than
//! [`LISTENING_GAP_SECONDS`] is a listening gap: analyzers could not deliver results, so it
//! stays unacknowledged until someone who performs analysis acknowledges it with a note.
//! A forward wall-clock jump while listeners were active is a listening gap of the run too
//! (`health::record_clock_jump`, M9); one acknowledgement covers all of a run's open gaps.
//!
//! Start, stop and acknowledgement are audited; heartbeats are technical writes.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Id, UtcTimestamp, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::health::clock_jumps;
use crate::policy::{LISTENING_GAP_SECONDS, NOTE_MAX_CHARS};
use crate::runtime_error::RuntimeError;

mod view;

use view::get_app_run as load_view;
pub use view::{
    AppRunView, get_app_run, list_app_runs, list_app_runs_page, pending_listening_gaps,
};

/// Audit action of an app start.
pub const APP_START: &str = "ops.app_start";
/// Audit action of an app stop.
pub const APP_STOP: &str = "ops.app_stop";
/// Audit action of a listening-gap acknowledgement (clinical stream).
pub const ACKNOWLEDGE_GAP: &str = "ops.acknowledge_gap";

/// Marker for app run identifiers.
pub enum AppRun {}
/// Identifies one run of the app.
pub type AppRunId = Id<AppRun>;

lis_core::text_enum! {
    /// How a run stopped.
    #[ts(export)]
    StopKind {
        /// Quit with nothing pending.
        Clean = "clean",
        /// Quit although the close warning listed pending work.
        QuitWithWarnings = "quit_with_warnings",
    }
}

/// What the engine knows about this start. Machine text from the build and the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStart {
    /// The app version, such as `0.4.0`.
    pub app_version: String,
    /// The operating system name and version.
    pub os_text: String,
    /// The web engine name and version, when the shell reports it.
    pub webview_text: Option<String>,
}

/// The outcome of [`start_run`], for the startup sequence and health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartedRun {
    /// The new run.
    pub run_id: AppRunId,
    /// The previous run never stopped.
    pub previous_unclean: bool,
    /// Seconds since the previous run was last seen; `None` on the first start.
    pub gap_seconds: Option<i64>,
    /// The app version differs from the previous run's.
    pub version_changed: bool,
    /// The web engine differs from the previous run's.
    pub webview_changed: bool,
}

/// Who did something, and when: an acknowledgement, withdrawal or acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Stamp {
    /// Who (principal ID text).
    pub by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub by_display: Option<String>,
    /// When.
    pub at: UtcTimestamp,
}

/// Input of `acknowledge_listening_gap`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AcknowledgeGapInput {
    /// The run that recorded the gap.
    pub run_id: AppRunId,
    /// What was done about results that could not arrive.
    pub note: String,
}

/// Run facts for health: open listening gaps, recent unclean stops, web-engine change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunFacts {
    /// Listening gaps not yet acknowledged: gaps before a start and clock gaps.
    pub unacknowledged_gaps: u32,
    /// Starts within the last 30 days whose previous run never stopped.
    pub unclean_stops_30d: u32,
    /// When the newest web-engine change was first seen; `None` if never.
    pub webview_changed_at: Option<UtcTimestamp>,
}

/// Unclean stops count towards health for this many days.
const UNCLEAN_WINDOW_DAYS: i64 = 30;

struct PreviousRun {
    id: AppRunId,
    last_seen_at: UtcTimestamp,
    stopped: bool,
    app_version: String,
    webview_text: Option<String>,
}

fn previous_run(conn: &Connection) -> Result<Option<PreviousRun>, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT id, last_seen_at, stopped_at IS NOT NULL, app_version, webview_text \
             FROM ops_app_runs ORDER BY started_at DESC, id DESC LIMIT 1",
        )?
        .query_row([], |row| {
            Ok(PreviousRun {
                id: row.get(0)?,
                last_seen_at: row.get(1)?,
                stopped: row.get(2)?,
                app_version: row.get(3)?,
                webview_text: row.get(4)?,
            })
        })
        .optional()?)
}

fn machine_text(field: &str, text: &str) -> Result<String, RuntimeError> {
    Ok(required_text(field, text, 200)?)
}

/// Records a new run at startup (§6.4 step 8) and audits `ops.app_start` on the security
/// stream. Engine-only: the engine runs it as a system principal.
pub fn start_run(
    ctx: &mut CommandContext<'_>,
    start: &RunStart,
) -> Result<StartedRun, RuntimeError> {
    let app_version = machine_text("appVersion", &start.app_version)?;
    let os_text = machine_text("osText", &start.os_text)?;
    let webview = start
        .webview_text
        .as_deref()
        .map(|text| machine_text("webviewText", text))
        .transpose()?;
    let previous = previous_run(ctx.tx())?;
    let now = ctx.now();
    let started = StartedRun {
        run_id: AppRunId::generate(),
        previous_unclean: previous.as_ref().is_some_and(|run| !run.stopped),
        gap_seconds: previous.as_ref().map(|run| {
            i64::try_from(now.saturating_duration_since(run.last_seen_at).as_secs())
                .unwrap_or(i64::MAX)
        }),
        version_changed: previous
            .as_ref()
            .is_some_and(|run| run.app_version != app_version),
        webview_changed: previous
            .as_ref()
            .is_some_and(|run| run.webview_text != webview),
    };
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_app_runs (id, started_at, last_seen_at, app_version, os_text, \
             webview_text, previous_run_id, previous_unclean, gap_seconds) \
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?
        .execute(params![
            started.run_id,
            now,
            app_version,
            os_text,
            webview,
            previous.as_ref().map(|run| run.id),
            started.previous_unclean,
            started.gap_seconds,
        ])?;
    let mut event = AuditEvent::new(Stream::Security, APP_START, Outcome::Succeeded)
        .record("app_run", started.run_id)
        .detail(
            "previous_unclean",
            AuditDetail::boolean(started.previous_unclean),
        )
        .detail(
            "version_changed",
            AuditDetail::boolean(started.version_changed),
        )
        .detail(
            "webview_changed",
            AuditDetail::boolean(started.webview_changed),
        );
    if let (Some(previous), Some(gap)) = (&previous, started.gap_seconds) {
        event = event
            .detail("previous_run_id", AuditDetail::id(previous.id))
            .detail("gap_seconds", AuditDetail::integer(gap));
    }
    ctx.audit(event)?;
    Ok(started)
}

/// Fails unless `run_id` exists and is still open.
fn ensure_open(conn: &Connection, run_id: AppRunId) -> Result<(), RuntimeError> {
    let stopped: bool = conn
        .prepare_cached("SELECT stopped_at IS NOT NULL FROM ops_app_runs WHERE id = ?1")?
        .query_row([run_id], |row| row.get(0))
        .optional()?
        .ok_or(RuntimeError::NotFound("app run"))?;
    if stopped {
        return Err(RuntimeError::RunStopped);
    }
    Ok(())
}

/// Moves the open run's `last_seen_at` to now: the light loop's technical heartbeat, so
/// the next start can measure the gap. Not audited.
pub fn heartbeat(ctx: &mut CommandContext<'_>, run_id: AppRunId) -> Result<(), RuntimeError> {
    ensure_open(ctx.tx(), run_id)?;
    ctx.tx()
        .prepare_cached("UPDATE ops_app_runs SET last_seen_at = ?2 WHERE id = ?1")?
        .execute(params![run_id, ctx.now()])?;
    Ok(())
}

/// Stops the open run, once, and audits `ops.app_stop` with the stop kind. The engine
/// runs it as the signed-in user, or as `sign-in-screen` when no one is signed in.
pub fn stop_run(
    ctx: &mut CommandContext<'_>,
    run_id: AppRunId,
    kind: StopKind,
) -> Result<(), RuntimeError> {
    ensure_open(ctx.tx(), run_id)?;
    ctx.tx()
        .prepare_cached(
            "UPDATE ops_app_runs SET stopped_at = ?2, last_seen_at = ?2, stop_kind = ?3 \
             WHERE id = ?1",
        )?
        .execute(params![run_id, ctx.now(), kind])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, APP_STOP, Outcome::Succeeded)
            .record("app_run", run_id)
            .detail("stop_kind", AuditDetail::code(kind.as_str())),
    )?;
    Ok(())
}

/// Acknowledges a run's listening gaps (`perform_analysis`; REQ-OPS-002): its gap
/// before the start when longer than [`LISTENING_GAP_SECONDS`], and its forward clock
/// jumps while listening, whichever nobody has acknowledged. The note is the audit reason
/// of `ops.acknowledge_gap` on the clinical stream.
pub fn acknowledge_gap(
    ctx: &mut CommandContext<'_>,
    input: AcknowledgeGapInput,
) -> Result<AppRunView, RuntimeError> {
    ctx.require(Capability::PerformAnalysis)?;
    let note = required_text("note", &input.note, NOTE_MAX_CHARS)?;
    let run = load_view(ctx.tx(), input.run_id)?;
    if !run.needs_acknowledgement {
        let had_gap = is_listening_gap(run.gap_seconds) || run.clock_gap_seconds.is_some();
        return Err(if had_gap {
            RuntimeError::GapAlreadyAcknowledged
        } else {
            RuntimeError::NoListeningGap
        });
    }
    let by = ctx.principal().actor().id_text();
    let start_gap =
        is_listening_gap(run.gap_seconds) && !start_gap_acknowledged(ctx.tx(), input.run_id)?;
    if start_gap {
        ctx.tx()
            .prepare_cached(
                "UPDATE ops_app_runs SET gap_acknowledged_by = ?2, gap_acknowledged_at = ?3 \
                 WHERE id = ?1",
            )?
            .execute(params![input.run_id, by, ctx.now()])?;
    }
    let clock_gap = clock_jumps::acknowledge_run_gaps(ctx, input.run_id, &by)?;
    let gap = if start_gap { run.gap_seconds } else { None };
    ctx.audit(
        AuditEvent::new(Stream::Clinical, ACKNOWLEDGE_GAP, Outcome::Succeeded)
            .record("app_run", input.run_id)
            .detail("gap_seconds", AuditDetail::integer(gap.unwrap_or_default()))
            .detail(
                "clock_gap_seconds",
                AuditDetail::integer(clock_gap.unwrap_or_default()),
            )
            .reason(note),
    )?;
    load_view(ctx.tx(), input.run_id)
}

fn start_gap_acknowledged(conn: &Connection, run_id: AppRunId) -> Result<bool, RuntimeError> {
    Ok(conn
        .prepare_cached("SELECT gap_acknowledged_at IS NOT NULL FROM ops_app_runs WHERE id = ?1")?
        .query_row([run_id], |row| row.get(0))?)
}

/// True when a gap is a listening gap.
fn is_listening_gap(gap_seconds: Option<i64>) -> bool {
    gap_seconds.is_some_and(|gap| gap > i64::from(LISTENING_GAP_SECONDS))
}

/// The run facts health evaluates at `now`.
pub fn run_facts(conn: &Connection, now: UtcTimestamp) -> Result<RunFacts, RuntimeError> {
    let (start_gaps, unclean_stops_30d): (u32, u32) = conn
        .prepare_cached(
            "SELECT \
             (SELECT count(*) FROM ops_app_runs WHERE gap_seconds > ?1 \
              AND gap_acknowledged_at IS NULL), \
             (SELECT count(*) FROM ops_app_runs WHERE previous_unclean = 1 \
              AND julianday(?2) - julianday(started_at) <= ?3)",
        )?
        .query_row(
            params![LISTENING_GAP_SECONDS, now, UNCLEAN_WINDOW_DAYS],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
    let webview_changed_at = conn
        .prepare_cached(
            "SELECT r.started_at FROM ops_app_runs r JOIN ops_app_runs p ON p.id = r.previous_run_id \
             WHERE r.webview_text IS NOT p.webview_text ORDER BY r.started_at DESC LIMIT 1",
        )?
        .query_row([], |row| row.get(0))
        .optional()?;
    Ok(RunFacts {
        unacknowledged_gaps: start_gaps + clock_jumps::unacknowledged_gaps(conn)?,
        unclean_stops_30d,
        webview_changed_at,
    })
}
