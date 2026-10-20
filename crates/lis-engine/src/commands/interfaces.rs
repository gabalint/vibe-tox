//! Analyzer configuration (REQ-INT-005, REQ-INT-009, REQ-OPS-001): list,
//! register, change the connection (with its peer allow-list) and enable or
//! disable instruments. All need `manage_configuration`; changes are audited
//! on the `security` stream by `lis-interfaces`.
//!
//! The listeners read their configuration when they start, so after every
//! committed change the engine restarts them (`restart_listeners`).
//! A restart stops every listener, including transfers in progress, and the
//! result says so.

use std::sync::Arc;

use lis_core::{Capability, CommandError};
use lis_interfaces::{
    Instrument, InterfaceError, RegisterInstrument, SetInstrumentEnabled,
    UpdateInstrumentConnection,
};
use serde::Serialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::domain_error;
use crate::registry::{Access, CommandSpec, spec};

const MANAGE: Access = Access::Capability(Capability::ManageConfiguration);

pub(crate) const LIST_INSTRUMENTS: CommandSpec =
    spec("list_instruments", "interfaces.list_instruments", MANAGE);
pub(crate) const REGISTER_INSTRUMENT: CommandSpec = spec(
    "register_instrument",
    "interface.register_instrument",
    MANAGE,
);
pub(crate) const UPDATE_INSTRUMENT_CONNECTION: CommandSpec = spec(
    "update_instrument_connection",
    "interface.update_connection",
    MANAGE,
);
pub(crate) const SET_INSTRUMENT_ENABLED: CommandSpec =
    spec("set_instrument_enabled", "interface.set_enabled", MANAGE);

const RESTARTED: &str = "Saved. The analyzer listeners were restarted to apply the change; \
     a transfer in progress was stopped. Check the listener status.";
const NOT_RESTARTED: &str = "Saved, but the analyzer listeners did not all restart. \
     Check the listener status; the change applies when they start.";

/// An instrument after a configuration change, and whether the listeners
/// restarted to apply it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstrumentChangeResult {
    /// The saved configuration.
    pub instrument: Instrument,
    /// True when every listener stopped and every enabled one started again.
    pub listeners_restarted: bool,
    /// What happened, for the user.
    pub message: String,
}

domain_error!(InterfaceError, InterfaceError::field_errors);

impl Engine {
    /// `list_instruments`: every analyzer configuration, by name.
    pub async fn list_instruments(&self) -> Result<Vec<Instrument>, CommandError> {
        self.run(&LIST_INSTRUMENTS, |call| async move {
            let principal = call.principal.clone();
            self.read(&call, move |conn| {
                lis_interfaces::list_instruments(conn, &principal)
            })
            .await
        })
        .await
    }

    /// `register_instrument`, then a listener restart.
    pub async fn register_instrument(
        &self,
        input: RegisterInstrument,
    ) -> Result<InstrumentChangeResult, CommandError> {
        self.run(&REGISTER_INSTRUMENT, |call| async move {
            let instrument = self
                .write(&call, move |ctx| {
                    lis_interfaces::register_instrument(ctx, input)
                })
                .await?;
            Ok(self.applied(instrument).await)
        })
        .await
    }

    /// `update_instrument_connection`, then a listener restart.
    pub async fn update_instrument_connection(
        &self,
        input: UpdateInstrumentConnection,
    ) -> Result<InstrumentChangeResult, CommandError> {
        self.run(&UPDATE_INSTRUMENT_CONNECTION, |call| async move {
            let instrument = self
                .write(&call, move |ctx| {
                    lis_interfaces::update_instrument_connection(ctx, input)
                })
                .await?;
            Ok(self.applied(instrument).await)
        })
        .await
    }

    /// `set_instrument_enabled`, then a listener restart.
    pub async fn set_instrument_enabled(
        &self,
        input: SetInstrumentEnabled,
    ) -> Result<InstrumentChangeResult, CommandError> {
        self.run(&SET_INSTRUMENT_ENABLED, |call| async move {
            let instrument = self
                .write(&call, move |ctx| {
                    lis_interfaces::set_instrument_enabled(ctx, input)
                })
                .await?;
            Ok(self.applied(instrument).await)
        })
        .await
    }

    /// Restarts the listeners after a committed change and reports it.
    async fn applied(&self, instrument: Instrument) -> InstrumentChangeResult {
        let listeners_restarted = self.restart_listeners().await;
        let message = if listeners_restarted {
            RESTARTED
        } else {
            NOT_RESTARTED
        };
        InstrumentChangeResult {
            instrument,
            listeners_restarted,
            message: message.to_owned(),
        }
    }

    /// Stops every analyzer listener and starts the enabled ones with the
    /// stored configuration (REQ-INT-009). Returns false, after logging why,
    /// when a listener did not stop cleanly or did not start; listener status
    /// shows which.
    async fn restart_listeners(&self) -> bool {
        let listeners = Arc::clone(&self.listeners);
        match tokio::task::spawn_blocking(move || listeners.restart_all()).await {
            Ok(Ok(())) => true,
            Ok(Err(error)) => {
                tracing::warn!(%error, "analyzer listeners did not all restart; see listener status");
                false
            }
            Err(_) => {
                tracing::error!("analyzer listener restart stopped unexpectedly");
                false
            }
        }
    }
}
