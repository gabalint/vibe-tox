//! Host health (REQ-OPS-008; M4 design brief D8, §6.1): gathers [`HealthInputs`] from every
//! owner and the host probes, evaluates them with `lis_ops::health::evaluate`, and records
//! state changes in the history. Host probes run daily or on refresh, off the async
//! runtime; free space is read at every evaluation.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lis_core::{CorrelationId, Principal, UtcTimestamp};
use lis_interfaces::{ListenerState, ListenerStatus};
use lis_ops::format::KeyId;
use lis_ops::health::{
    CheckResult, ClockFacts, DiskFacts, HealthInputs, HealthState, ListenerFacts, OutboxFacts,
};
use lis_ops::host::{HostFacts, OsIdentity};
use lis_ops::policy::OperationsSettings;
use lis_store::DATABASE_FILE;
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use super::{Operations, OpsError, locked};
use crate::keep_awake::KeepAwakeView;

/// Host probes (OS, disk encryption, time synchronization) are repeated this often.
const PROBE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// The system principal of health history rows.
const COMPONENT: &str = "ops-health";

/// What evaluation keeps between steps.
#[derive(Debug, Default)]
pub(crate) struct HealthCache {
    /// The newest probe and when it ran.
    host: Option<(HostFacts, Instant)>,
    /// Findings of the newest chain and anchor verification.
    pub(crate) chain_findings: Option<u32>,
    /// When `refresh_host_health` last probed.
    pub(crate) last_refresh: Option<Instant>,
}

/// `get_host_health` and `refresh_host_health` answer this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostHealthView {
    /// The worst check, with `unknown` counted as a warning.
    pub overall: HealthState,
    /// Every check, in table order.
    pub checks: Vec<CheckResult>,
    /// This computer and app.
    pub host: HostView,
    /// The sleep inhibitor.
    pub keep_awake: KeepAwakeView,
}

/// This computer and app, as health shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostView {
    /// `windows`, `macos` or `linux`.
    pub os: String,
    /// The OS version; `unknown` when it could not be read.
    pub os_version: String,
    /// The Windows edition.
    pub edition: Option<String>,
    /// The CPU architecture.
    pub arch: String,
    /// The web engine.
    pub webview: Option<String>,
    /// The app version.
    pub app_version: String,
    /// The data folder.
    pub data_folder: String,
}

/// Evaluates every check now; `reprobe` repeats the host probes first.
pub(crate) async fn current(
    ops: &Arc<Operations>,
    reprobe: bool,
) -> Result<HostHealthView, OpsError> {
    let host = host_facts(ops, reprobe).await?;
    let key_id = locked(&ops.key)
        .key
        .as_ref()
        .map(|key| lis_ops::format::BackupKey::derive(key).key_id());
    let listeners = listener_facts(ops);
    let data_dir = ops.data_dir.clone();
    let chain_findings = locked(&ops.health).chain_findings;
    let now = ops.clock.now();
    let facts = host.clone();
    let (inputs, settings) = ops
        .db
        .read(move |conn| {
            gather(
                conn,
                now,
                &facts,
                key_id,
                &data_dir,
                listeners,
                chain_findings,
            )
        })
        .await?;
    let checks = lis_ops::health::evaluate(&inputs, &settings, now);
    Ok(HostHealthView {
        overall: lis_ops::health::overall(checks.iter().map(|check| check.state)),
        checks,
        host: host_view(&host, ops),
        keep_awake: ops.keep_awake(),
    })
}

/// Evaluates and appends a history row for each check whose state changed.
pub(crate) async fn evaluate_and_record(
    ops: &Arc<Operations>,
    reprobe: bool,
) -> Result<HostHealthView, OpsError> {
    let view = current(ops, reprobe).await?;
    let checks = view.checks.clone();
    let appended = ops
        .db
        .write(
            Principal::system(COMPONENT, []),
            CorrelationId::generate(),
            move |ctx| lis_ops::health::record_state_changes(ctx, &checks),
        )
        .await?;
    if appended > 0 {
        tracing::info!(
            changed = appended,
            overall = view.overall.as_str(),
            "health state changed"
        );
    }
    Ok(view)
}

/// The cached probe, repeated when older than a day or when `reprobe`.
async fn host_facts(ops: &Arc<Operations>, reprobe: bool) -> Result<HostFacts, OpsError> {
    let cached = locked(&ops.health)
        .host
        .clone()
        .filter(|(_, at)| !reprobe && at.elapsed() < PROBE_INTERVAL);
    let mut facts = match cached {
        Some((facts, _)) => facts,
        None => {
            let data_dir = ops.data_dir.clone();
            let facts = tokio::task::spawn_blocking(move || lis_ops::host::probe(&data_dir))
                .await
                .map_err(|_| OpsError::Task)?;
            locked(&ops.health).host = Some((facts.clone(), Instant::now()));
            facts
        }
    };
    facts.available_bytes = lis_ops::host::available_bytes(&ops.data_dir);
    Ok(facts)
}

fn listener_facts(ops: &Operations) -> ListenerFacts {
    let statuses = ops.listeners.status_snapshot();
    let count = |wanted: &dyn Fn(&ListenerStatus) -> bool| {
        let matching = statuses.iter().filter(|status| wanted(status)).count();
        u32::try_from(matching).unwrap_or(u32::MAX)
    };
    let sum = |files: fn(&ListenerStatus) -> Option<u32>| {
        statuses
            .iter()
            .filter_map(files)
            .fold(0_u32, u32::saturating_add)
    };
    ListenerFacts {
        enabled_in_error: count(&ListenerStatus::in_error),
        // Watched folders (F13, F14): counted warnings, not errors.
        files_waiting: sum(|status| status.files_waiting),
        files_too_large: sum(|status| status.files_too_large),
        active: count(&|status| {
            matches!(
                status.state,
                ListenerState::Listening | ListenerState::Connected | ListenerState::Receiving
            )
        }),
        keep_awake: ops.keep_awake().state,
    }
}

/// Every owner's facts at `now`, read on one connection.
#[expect(
    clippy::too_many_arguments,
    reason = "the inputs that do not come from the database, gathered by the caller"
)]
fn gather(
    conn: &Connection,
    now: UtcTimestamp,
    host: &HostFacts,
    key_id: Option<KeyId>,
    data_dir: &Path,
    listeners: ListenerFacts,
    audit_chain_findings: Option<u32>,
) -> Result<(HealthInputs, OperationsSettings), OpsError> {
    let settings = lis_ops::settings::current(conn)?;
    let clock = ClockFacts {
        last_backward_jump_at: lis_ops::health::last_backward_jump(conn)?,
        newest_record_at: newest_record_at(conn)?,
        time_sync: host.time_sync,
    };
    let inputs = HealthInputs {
        backup: lis_ops::backup::health_facts(conn, now, key_id)?,
        anchors: lis_ops::anchors::get_anchor_status(conn, now)?.health_facts(),
        audit_chain_findings,
        disk: DiskFacts {
            available_bytes: host.available_bytes,
            database_bytes: database_bytes(data_dir),
            growth_bytes_per_day: lis_ops::backup::snapshot_growth_per_day(conn, now)?,
        },
        disk_encryption: host.disk_encryption,
        os: host.os.clone(),
        runs: lis_ops::runs::run_facts(conn, now)?,
        listeners,
        clock,
        outbox: outbox_facts(conn, now)?,
        version: lis_ops::updates::version_facts(conn)?,
        recovery_open: lis_ops::recovery::is_open(conn)?,
        attestations: lis_ops::health::valid_attestations(conn, now)?,
    };
    Ok((inputs, settings))
}

/// The newest audit `recorded_at` of any stream; a clock behind it is wrong.
fn newest_record_at(conn: &Connection) -> Result<Option<UtcTimestamp>, OpsError> {
    let mut newest = None;
    for head in lis_audit::heads(conn)? {
        newest = newest.max(lis_audit::event_recorded_at(conn, head.stream, head.seq)?);
    }
    Ok(newest)
}

fn outbox_facts(conn: &Connection, now: UtcTimestamp) -> Result<OutboxFacts, OpsError> {
    let dead = lis_store::outbox::dead_letters(conn, 500)?.len();
    let oldest = lis_store::outbox::backlog(conn, now)?
        .into_iter()
        .map(|topic| topic.oldest_age)
        .max();
    Ok(OutboxFacts {
        dead_letters: u32::try_from(dead).unwrap_or(u32::MAX),
        oldest_pending_age: oldest,
    })
}

/// The database and its write-ahead log, in bytes.
fn database_bytes(data_dir: &Path) -> u64 {
    ["", "-wal"]
        .into_iter()
        .filter_map(|suffix| {
            std::fs::metadata(data_dir.join(format!("{DATABASE_FILE}{suffix}"))).ok()
        })
        .map(|metadata| metadata.len())
        .sum()
}

fn host_view(host: &HostFacts, ops: &Operations) -> HostView {
    let (os_version, edition) = match &host.os {
        Some(OsIdentity::Windows(windows)) => (
            windows
                .display_version
                .clone()
                .unwrap_or_else(|| windows.build.to_string()),
            Some(windows.edition_id.clone()),
        ),
        Some(OsIdentity::Macos(macos)) => (macos.product_version.clone(), None),
        Some(OsIdentity::Linux(linux)) => (
            linux
                .pretty_name
                .clone()
                .unwrap_or_else(|| format!("{} {}", linux.id, linux.version_id)),
            None,
        ),
        None => ("unknown".to_owned(), None),
    };
    HostView {
        os: std::env::consts::OS.to_owned(),
        os_version,
        edition,
        arch: std::env::consts::ARCH.to_owned(),
        webview: ops.webview.clone(),
        app_version: crate::engine::APP_VERSION.to_owned(),
        data_folder: ops.data_dir.display().to_string(),
    }
}
