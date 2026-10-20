use lis_core::{InstrumentId, UtcTimestamp};
use serde::Serialize;
use ts_rs::TS;

/// Runtime listener lifecycle, safe to show without a signed-in user.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ListenerState {
    /// Not running, whether by configuration or supervisor shutdown.
    Disabled,
    /// TCP socket is bound and awaiting a connection.
    Listening,
    /// Transport is connected and no transfer is active.
    Connected,
    /// A durable receipt is accepting frames.
    Receiving,
    /// Setup or execution failed and needs attention.
    Error,
    /// An enabled result-file instrument without a watched folder: its files
    /// are loaded by hand (result-files brief §6).
    ManualOnly,
}

/// PHI-free status detail for an instrument listener.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ListenerDetailCode {
    /// Instrument configuration is disabled.
    ConfigurationDisabled,
    /// Supervisor stopped the listener.
    ListenerStopped,
    /// Transport could not be opened.
    OpenFailed,
    /// Transport I/O failed.
    TransportFailed,
    /// Durable persistence failed.
    PersistenceFailed,
    /// ASTM driver or action sequencing failed.
    ProtocolFailed,
    /// AU test ordering was requested; automatic requisition lookup is not enabled.
    AuQueryUnsupported,
    /// Connected peer did not send ENQ before the establishment deadline.
    EstablishmentTimeout,
    /// A connection from an unconfigured peer address was rejected.
    UnexpectedPeerRejected,
    /// The watched folder of a result-file instrument is missing or cannot be read.
    FolderUnavailable,
    /// A file in the watched folder has waited more than 5 minutes to be read.
    FilesWaiting,
    /// A file in the watched folder was over the size limit and not read (F14).
    FileTooLarge,
}

/// Current runtime state for one configured instrument.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ListenerStatus {
    /// Configured instrument.
    pub instrument_id: InstrumentId,
    /// Current lifecycle state.
    pub state: ListenerState,
    /// Last byte, connection, or state-change time when known.
    pub last_activity_at: Option<UtcTimestamp>,
    /// Code-defined detail with no payload or patient data.
    pub detail_code: Option<ListenerDetailCode>,
    /// For a watched folder, the files waiting more than 5 minutes; absent for
    /// an ASTM listener.
    pub files_waiting: Option<u32>,
    /// For a watched folder, the files over the size limit found since the
    /// app started; absent for an ASTM listener.
    pub files_too_large: Option<u32>,
}

impl ListenerStatus {
    /// True when the listener or watched folder is in error (REQ-OPS-008).
    /// Files waiting or too large are counted warnings, not errors.
    #[must_use]
    pub fn in_error(&self) -> bool {
        self.state == ListenerState::Error
    }
}

/// A status as a listener or the folder poller reports it (F13).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatusReport {
    /// The lifecycle state.
    pub state: ListenerState,
    /// Why it is not working normally, when it is not.
    pub detail_code: Option<ListenerDetailCode>,
    /// Files waiting more than 5 minutes; `None` without a watched folder.
    pub files_waiting: Option<u32>,
    /// Files over the size limit found since the app started; `None`
    /// without a watched folder.
    pub files_too_large: Option<u32>,
}
