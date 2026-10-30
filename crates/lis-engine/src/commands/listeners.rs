//! Analyzer listener status: public, instrument names and states only, no
//! patient data (REQ-OPS-013). Shown at the sign-in screen and in the top bar.
//! A result-file instrument's watched folder shows here like a listener.

use std::collections::BTreeMap;

use lis_core::{CommandError, InstrumentId, UtcTimestamp};
use lis_interfaces::{ListenerDetailCode, ListenerState};
use serde::Serialize;
use ts_rs::TS;

use crate::Engine;
use crate::registry::LIST_LISTENER_STATUS;

/// One analyzer listener as the UI shows it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListenerStatusView {
    /// The configured instrument.
    pub instrument_id: InstrumentId,
    /// Its display name.
    pub instrument_name: String,
    /// The listener's lifecycle state.
    pub state: ListenerState,
    /// The last connection, byte or state change, when known.
    pub last_activity_at: Option<UtcTimestamp>,
    /// Why the listener is not working normally, when it is not.
    pub detail: Option<ListenerDetailCode>,
    /// For a watched folder, the files waiting more than 5 minutes (result-files brief §6).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub files_waiting: Option<u32>,
    /// For a watched folder, the files over the size limit found since the app started
    /// (F14); not read, each an aborted receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub files_too_large: Option<u32>,
    /// The latest error since the app started, kept after the listener recovers.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_error: Option<ListenerDetailCode>,
    /// When that error began.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_error_at: Option<UtcTimestamp>,
}

impl Engine {
    /// `list_listener_status` (public): every configured instrument with its
    /// listener state, ordered by name. An instrument without a running
    /// listener shows as disabled.
    pub async fn list_listener_status(&self) -> Result<Vec<ListenerStatusView>, CommandError> {
        self.run(&LIST_LISTENER_STATUS, |call| async move {
            let names = self.read(&call, lis_interfaces::instrument_names).await?;
            let mut statuses: BTreeMap<_, _> = self
                .listeners
                .status_snapshot()
                .into_iter()
                .map(|status| (status.instrument_id, status))
                .collect();
            Ok(names
                .into_iter()
                .map(|(instrument_id, instrument_name)| {
                    let status = statuses.remove(&instrument_id);
                    ListenerStatusView {
                        instrument_id,
                        instrument_name,
                        state: status.as_ref().map_or(ListenerState::Disabled, |s| s.state),
                        last_activity_at: status.as_ref().and_then(|s| s.last_activity_at),
                        detail: status.as_ref().and_then(|s| s.detail_code),
                        files_waiting: status.as_ref().and_then(|s| s.files_waiting),
                        files_too_large: status.as_ref().and_then(|s| s.files_too_large),
                        last_error: status.as_ref().and_then(|s| s.last_error),
                        last_error_at: status.and_then(|s| s.last_error_at),
                    }
                })
                .collect())
        })
        .await
    }
}
