use lis_core::{Id, InstrumentId, UtcTimestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

mod au;
mod connection;
mod files;
mod serial;

pub use au::AuConnectionSettings;
pub use connection::{ConnectionSettings, InstrumentTransport};
pub use files::{
    FileFacts, FileOrigin, FileSource, PresentFile, ReceiptFileView, ReceiveResultFileInput,
    SetResultFolder, WatchedFolder,
};
pub use serial::{DataBits, Parity, StopBits};

/// Marker type for a durable inbound transfer receipt.
pub enum ReceiptMarker {}
/// Identifier allocated by the LIS for one inbound transfer.
pub type ReceiptId = Id<ReceiptMarker>;

/// A configured analyzer connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "InstrumentConfiguration")]
pub struct Instrument {
    /// Stable analyzer identity.
    pub id: InstrumentId,
    /// Short unique laboratory code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Active transport settings.
    pub connection: ConnectionSettings,
    /// Canonical path of the watched folder of a result-file instrument;
    /// absent when files are only loaded by hand (F13).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result_folder: Option<String>,
    /// Whether the supervisor should start this listener, or the folder
    /// poller watch this instrument's folder.
    pub enabled: bool,
    /// Optimistic concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// Who registered it and when; absent when it was registered before this
    /// was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub created: Option<ChangeStamp>,
    /// Who last changed its configuration and when; absent when it has not
    /// changed since it was registered, or changed before this was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub changed: Option<ChangeStamp>,
}

/// Who made a configuration change, and when.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeStamp {
    /// The acting principal's ID.
    pub by: String,
    /// Current display name resolved by the engine from the account owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by_display: Option<String>,
    /// When the change was committed.
    pub at: UtcTimestamp,
}

/// Input for registering an analyzer connection.
#[derive(Clone, Debug, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, rename = "RegisterInstrumentInput")]
pub struct RegisterInstrument {
    /// Short unique laboratory code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Initial transport settings.
    pub connection: ConnectionSettings,
    /// Whether listening begins on the next supervisor start.
    pub enabled: bool,
}

/// Input for changing only an analyzer's connection settings.
#[derive(Clone, Debug, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, rename = "UpdateInstrumentConnectionInput")]
pub struct UpdateInstrumentConnection {
    /// Analyzer to change.
    pub instrument_id: InstrumentId,
    /// Revision observed by the caller.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Replacement transport settings.
    pub connection: ConnectionSettings,
}

/// Input for enabling or disabling an analyzer listener.
#[derive(Clone, Debug, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, rename = "SetInstrumentEnabledInput")]
pub struct SetInstrumentEnabled {
    /// Analyzer to change.
    pub instrument_id: InstrumentId,
    /// Revision observed by the caller.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Desired listener state after the next supervisor refresh.
    pub enabled: bool,
}

/// Durable state of one inbound transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    /// Frames may still be appended.
    Receiving,
    /// A complete synced artifact is referenced.
    Complete,
    /// The transfer ended without a complete message.
    Aborted,
    /// Startup reconciliation found an unfinished prior-run transfer.
    Interrupted,
}

impl ReceiptState {
    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "receiving" => Ok(Self::Receiving),
            "complete" => Ok(Self::Complete),
            "aborted" => Ok(Self::Aborted),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// Code-defined reason that an incomplete transfer became terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbortCode {
    /// ASTM retry limit reached.
    RetryLimitExceeded,
    /// ASTM receive timer expired.
    ReceiveTimeout,
    /// EOT arrived with an unfinished record.
    IncompleteRecord,
    /// EOT arrived before any record.
    EmptyTransfer,
    /// Configured maximum message size was exceeded.
    MessageTooLarge,
    /// Bounded wire buffer was exceeded.
    InputBufferExceeded,
    /// An accepted frame contained no application text.
    EmptyFrame,
    /// The transfer exceeded the accepted-frame bound.
    FrameLimitExceeded,
    /// The transfer exceeded the durable exact-frame-byte bound.
    WireBytesExceeded,
    /// The transport closed before message completion.
    ConnectionLost,
    /// The transport returned a non-timeout I/O failure.
    TransportError,
    /// Listener shutdown interrupted a transfer.
    ListenerStopped,
    /// Driver or action sequencing failed.
    ProtocolError,
    /// Startup found a transfer left open by an earlier run.
    StartupInterrupted,
    /// A result file was over [`crate::MAX_RESULT_FILE_BYTES`] and was not read (F14).
    FileTooLarge,
}

impl AbortCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RetryLimitExceeded => "retry_limit_exceeded",
            Self::ReceiveTimeout => "receive_timeout",
            Self::IncompleteRecord => "incomplete_record",
            Self::EmptyTransfer => "empty_transfer",
            Self::MessageTooLarge => "message_too_large",
            Self::InputBufferExceeded => "input_buffer_exceeded",
            Self::EmptyFrame => "empty_frame",
            Self::FrameLimitExceeded => "frame_limit_exceeded",
            Self::WireBytesExceeded => "wire_bytes_exceeded",
            Self::ConnectionLost => "connection_lost",
            Self::TransportError => "transport_error",
            Self::ListenerStopped => "listener_stopped",
            Self::ProtocolError => "protocol_error",
            Self::StartupInterrupted => "startup_interrupted",
            Self::FileTooLarge => "file_too_large",
        }
    }

    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "retry_limit_exceeded" => Ok(Self::RetryLimitExceeded),
            "receive_timeout" => Ok(Self::ReceiveTimeout),
            "incomplete_record" => Ok(Self::IncompleteRecord),
            "empty_transfer" => Ok(Self::EmptyTransfer),
            "message_too_large" => Ok(Self::MessageTooLarge),
            "input_buffer_exceeded" => Ok(Self::InputBufferExceeded),
            "empty_frame" => Ok(Self::EmptyFrame),
            "frame_limit_exceeded" => Ok(Self::FrameLimitExceeded),
            "wire_bytes_exceeded" => Ok(Self::WireBytesExceeded),
            "connection_lost" => Ok(Self::ConnectionLost),
            "transport_error" => Ok(Self::TransportError),
            "listener_stopped" => Ok(Self::ListenerStopped),
            "protocol_error" => Ok(Self::ProtocolError),
            "startup_interrupted" => Ok(Self::StartupInterrupted),
            "file_too_large" => Ok(Self::FileTooLarge),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// One durable transfer receipt, separate from semantic result acceptance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Receipt {
    /// LIS-assigned receipt identity.
    pub id: ReceiptId,
    /// Analyzer attributed as the source.
    pub instrument_id: InstrumentId,
    /// Transport used for the transfer.
    pub transport: InstrumentTransport,
    /// TCP peer address; absent for serial and files.
    pub peer: Option<String>,
    /// When the durable receipt began.
    pub started_at: UtcTimestamp,
    /// When a terminal state was committed.
    pub completed_at: Option<UtcTimestamp>,
    /// Current receipt state.
    pub state: ReceiptState,
    /// Code-defined terminal reason for incomplete transfers.
    pub abort_reason: Option<AbortCode>,
    /// Count of exact accepted frames; 0 for files.
    pub frame_count: u32,
    /// SHA-256 of the assembled message bytes when complete.
    pub message_sha256: Option<String>,
    /// Content-addressed artifact digest when complete.
    pub artifact_digest: Option<String>,
    /// Earlier complete receipt with identical bytes from this instrument.
    pub duplicate_of: Option<ReceiptId>,
}

/// Exact bytes of one accepted ASTM frame in durable order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptFrame {
    /// Owning receipt.
    pub receipt_id: ReceiptId,
    /// Zero-based durable frame order.
    pub ordinal: u32,
    /// Exact bytes from STX through CR/LF.
    pub bytes: Vec<u8>,
}
