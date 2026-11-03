//! The analyzer listeners around an in-app update (REQ-OPS-006, REQ-OPS-013; ADR 0020).
//! `stop_listeners_for_update` waits for the messages being received to finish, then stops
//! every listener; no instrument's enabled setting changes. A failed installation starts
//! them again at once. The new version starts them as every start does, and the sign-in
//! screen shows how many opened and which did not.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lis_core::{CommandError, ErrorCode, InstrumentId};
use lis_interfaces::{ListenerState, ListenerSupervisor};
use lis_ops::updates::{ListenerRestart, record_listeners_restarted};
use lis_store::Database;
use serde::Serialize;
use ts_rs::TS;

use super::{INSTALL_UPDATE, STOP_LISTENERS_FOR_UPDATE};
use crate::ListenerStatusView;
use crate::engine::Engine;
use crate::ops_worker::startup::lifecycle_write;
use crate::pipeline::Failure;
use crate::registry::GET_LISTENER_RESTART;

/// How long the stop waits for the messages being received.
const RECEIVING_WAIT: Duration = Duration::from_secs(60);
/// How often it checks.
const RECEIVING_POLL: Duration = Duration::from_millis(250);

/// What `stop_listeners_for_update` stopped.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StoppedListenersView {
    /// The listeners that were running and stopped.
    pub stopped: u32,
}

/// The sign-in screen's summary after the first start of a new version
/// (`get_listener_restart`, public; instrument names and states only).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListenerRestartView {
    /// The listeners that opened again.
    pub listening: u32,
    /// The enabled listeners that did not open, with their current state and reason.
    pub failed: Vec<ListenerStatusView>,
}

/// Which enabled listeners a start opened and which it could not open.
#[derive(Debug, Clone)]
pub(crate) struct RestartedListeners {
    listening: usize,
    failed: Vec<InstrumentId>,
}

impl RestartedListeners {
    fn of(listeners: &ListenerSupervisor) -> Self {
        let running = listeners.running();
        let failed = listeners
            .status_snapshot()
            .into_iter()
            .filter(|status| {
                status.state == ListenerState::Error && !running.contains(&status.instrument_id)
            })
            .map(|status| status.instrument_id)
            .collect();
        Self {
            listening: running.len(),
            failed,
        }
    }
}

/// After the first start of a new version: which enabled listeners opened again,
/// audited as `ops.restart_listeners_after_update`. `None` when the version is unchanged.
pub(crate) async fn listeners_after_new_version(
    db: &Database,
    listeners: &ListenerSupervisor,
    version_changed: bool,
) -> Option<RestartedListeners> {
    if !version_changed {
        return None;
    }
    let restarted = RestartedListeners::of(listeners);
    let (listening, failed) = (restarted.listening, restarted.failed.len());
    let recorded = lifecycle_write(db, move |ctx| {
        record_listeners_restarted(ctx, ListenerRestart::NewVersion, listening, failed)
    })
    .await;
    if let Err(error) = recorded {
        tracing::error!(%error, "listener restart after the update not recorded");
    }
    Some(restarted)
}

impl Engine {
    /// `stop_listeners_for_update` (`manage_configuration`): waits up to a minute for every
    /// message being received to be received and acknowledged, then stops every analyzer
    /// listener. Each instrument's enabled setting stays as it is. Audited as
    /// `ops.stop_listeners_for_update`.
    pub async fn stop_listeners_for_update(&self) -> Result<StoppedListenersView, CommandError> {
        self.run(&STOP_LISTENERS_FOR_UPDATE, |call| async move {
            self.authorize(&call).await?;
            self.wait_for_receiving().await?;
            let stopped = self.listeners.running().len();
            self.write(&call, move |ctx| {
                lis_ops::updates::record_listeners_stopped(ctx, stopped)
            })
            .await?;
            self.listeners_stopped_for_update
                .store(true, Ordering::SeqCst);
            let listeners = Arc::clone(&self.listeners);
            match tokio::task::spawn_blocking(move || listeners.stop_all()).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!(%error, "analyzer listeners did not stop cleanly")
                }
                Err(_) => tracing::error!("analyzer listener stop ended unexpectedly"),
            }
            tracing::info!(stopped, "analyzer listeners stopped for the update");
            Ok(StoppedListenersView {
                stopped: u32::try_from(stopped).unwrap_or(u32::MAX),
            })
        })
        .await
    }

    /// Starts the enabled listeners again when an installation did not happen after
    /// [`stop_listeners_for_update`](Self::stop_listeners_for_update); does nothing
    /// otherwise. Audited as `ops.restart_listeners_after_update`.
    pub async fn restart_listeners_after_failed_update(&self) -> Result<(), CommandError> {
        if !self
            .listeners_stopped_for_update
            .swap(false, Ordering::SeqCst)
        {
            return Ok(());
        }
        let listeners = Arc::clone(&self.listeners);
        let restarted = tokio::task::spawn_blocking(move || {
            if let Err(error) = listeners.start_all() {
                tracing::warn!(%error, "some analyzer listeners did not start; see listener status");
            }
            RestartedListeners::of(&listeners)
        })
        .await
        .map_err(|_| Failure::Internal("listener restart ended unexpectedly".to_owned()));
        self.run(&INSTALL_UPDATE, |call| async move {
            let restarted = restarted?;
            let (listening, failed) = (restarted.listening, restarted.failed.len());
            self.write(&call, move |ctx| {
                record_listeners_restarted(
                    ctx,
                    ListenerRestart::InstallNotCompleted,
                    listening,
                    failed,
                )
            })
            .await
        })
        .await
    }

    /// `get_listener_restart` (public): after the first start of a new version, how many
    /// listeners opened again and which did not; `None` otherwise.
    pub async fn get_listener_restart(&self) -> Result<Option<ListenerRestartView>, CommandError> {
        self.run(&GET_LISTENER_RESTART, |call| async move {
            let Some(restarted) = &self.listeners_after_update else {
                return Ok(None);
            };
            let failed = self
                .listener_views(&call)
                .await?
                .into_iter()
                .filter(|view| restarted.failed.contains(&view.instrument_id))
                .collect();
            Ok(Some(ListenerRestartView {
                listening: u32::try_from(restarted.listening).unwrap_or(u32::MAX),
                failed,
            }))
        })
        .await
    }

    /// Refuses once a message is still being received after [`RECEIVING_WAIT`].
    async fn wait_for_receiving(&self) -> Result<(), Failure> {
        let deadline = Instant::now() + RECEIVING_WAIT;
        while self
            .listeners
            .status_snapshot()
            .iter()
            .any(|status| status.state == ListenerState::Receiving)
        {
            if Instant::now() >= deadline {
                return Err(Failure::refused(
                    ErrorCode::Conflict,
                    "An analyzer is still sending results. Try again when it finishes.",
                ));
            }
            tokio::time::sleep(RECEIVING_POLL).await;
        }
        Ok(())
    }
}
