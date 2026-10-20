use std::time::Duration;

use thiserror::Error;

/// Default maximum application text accepted in one transfer (one mebibyte).
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 1024 * 1024;
/// Maximum accepted frames in one transfer, independent of application-text size.
pub const DEFAULT_MAX_FRAMES_PER_TRANSFER: u32 = 8_192;
/// Maximum exact frame bytes accepted: the text limit plus 50 KiB of framing.
pub const DEFAULT_MAX_WIRE_BYTES_PER_TRANSFER: usize = DEFAULT_MAX_MESSAGE_BYTES + 50 * 1024;
/// Receiver inter-frame and persistence timeout requested from the caller.
pub const RECEIVE_TIMEOUT: Duration = Duration::from_secs(30);

/// Opaque identity for one frame persistence request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FrameId(pub(crate) u64);

/// Opaque identity used to ignore expiry events from replaced timers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimerId(pub(crate) u64);

/// A link response for the caller to transmit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlResponse {
    /// Positive acknowledgement.
    Ack,
    /// Negative acknowledgement.
    Nak,
}

/// Why a transfer ended without exposing a partial message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortReason {
    /// Six consecutive malformed or out-of-sequence attempts were rejected.
    RetryLimitExceeded,
    /// The current receiver timer expired.
    ReceiveTimeout,
    /// EOT arrived while record text remained unterminated.
    IncompleteRecord,
    /// EOT arrived before any complete record was accepted.
    EmptyTransfer,
    /// Accepted frame text would exceed the configured message limit.
    MessageTooLarge,
    /// Buffered unprocessed wire input exceeded the bounded receive capacity.
    InputBufferExceeded,
    /// A frame contained no application text.
    EmptyFrame,
    /// Accepted frame count would exceed the transfer bound.
    FrameLimitExceeded,
    /// Accepted exact frame bytes would exceed the transfer bound.
    WireBytesExceeded,
}

/// Work that the I/O and persistence owner must perform in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Transmit an ACK or NAK control byte.
    Send(ControlResponse),
    /// Durably store the exact validated frame bytes under this opaque identity.
    PersistFrame {
        /// Identity to return only after the bytes are durably committed.
        frame_id: FrameId,
        /// Exact bytes from STX through CR/LF.
        bytes: Vec<u8>,
    },
    /// Expose a complete transfer as unterminated E1394 record byte strings.
    MessageComplete(Vec<Vec<u8>>),
    /// Discard the partial transfer and surface an operational failure.
    TransferAborted(AbortReason),
    /// Start or replace the receive timer with this identity and duration.
    StartTimer {
        /// Identity returned in the expiry event.
        timer_id: TimerId,
        /// Duration the external scheduler should use.
        duration: Duration,
    },
    /// Stop the active receive timer, if any.
    StopTimer,
}

impl Action {
    /// Returns the identity to confirm when this action requests persistence.
    #[must_use]
    pub const fn persistence_id(&self) -> Option<FrameId> {
        match self {
            Self::PersistFrame { frame_id, .. } => Some(*frame_id),
            Self::Send(_)
            | Self::MessageComplete(_)
            | Self::TransferAborted(_)
            | Self::StartTimer { .. }
            | Self::StopTimer => None,
        }
    }
}

/// Inputs from the byte transport, persistence owner, and external scheduler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiveEvent {
    /// One or more bytes arrived; chunk boundaries carry no meaning.
    Bytes(Vec<u8>),
    /// The exact frame associated with this identity is durably committed.
    Persisted(FrameId),
    /// A previously requested timer expired.
    TimerExpired(TimerId),
}

/// Misuse or exhaustion of an opaque driver identity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DriverError {
    /// No outstanding persistence request has this identity.
    #[error("ASTM persistence confirmation does not match the pending frame")]
    UnknownFrameId,
    /// The driver's monotonic frame identity space was exhausted.
    #[error("ASTM frame identity space is exhausted")]
    FrameIdExhausted,
    /// The driver's monotonic timer identity space was exhausted.
    #[error("ASTM timer identity space is exhausted")]
    TimerIdExhausted,
}
