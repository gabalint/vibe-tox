use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

/// Stable, PHI-free listener failure codes used in status and logs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListenerErrorCode {
    /// The configured transport could not be opened.
    OpenFailed,
    /// The connection failed while receiving.
    TransportFailed,
    /// Durable persistence failed.
    PersistenceFailed,
    /// The ASTM driver contract was violated or exhausted.
    ProtocolFailed,
    /// A TCP peer did not send ENQ within the establishment window.
    EstablishmentTimeout,
    /// A TCP peer address was not configured for the instrument.
    UnexpectedPeer,
}

/// Failures from instrument commands, receipt commands, and queries.
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    /// The store, audit trail, artifact store, or authorization check failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// No instrument with the requested ID exists.
    #[error("instrument not found")]
    InstrumentNotFound,
    /// No receipt with the requested ID exists.
    #[error("receipt not found")]
    ReceiptNotFound,
    /// The instrument code is malformed or too long.
    #[error("instrument code is invalid")]
    InvalidInstrumentCode,
    /// The display name is empty, too long, or contains control characters.
    #[error("instrument name is invalid")]
    InvalidInstrumentName,
    /// The TCP bind address is not a loopback or private LAN address.
    #[error("TCP bind address must be a loopback or private LAN address")]
    InvalidTcpBindAddress,
    /// The TCP port is zero.
    #[error("TCP port is invalid")]
    InvalidTcpPort,
    /// TCP peer allowlist is empty, duplicated, or outside lab network ranges.
    #[error("TCP allowed peers must be unique loopback or private LAN addresses")]
    InvalidTcpAllowedPeers,
    /// The serial port path is empty, too long, or contains control characters.
    #[error("serial port path is invalid")]
    InvalidSerialPort,
    /// The configured baud rate is outside the supported range.
    #[error("serial baud rate is invalid")]
    InvalidBaudRate,
    /// AU framing, format, timing or line settings do not fit the qualified profile.
    #[error("AU settings do not match the supported online parameter profile")]
    InvalidAuConfiguration,
    /// Another instrument already uses the code.
    #[error("instrument code is already in use")]
    InstrumentCodeInUse,
    /// Another instrument already owns the connection endpoint.
    #[error("instrument connection is already in use")]
    ConnectionInUse,
    /// The caller's expected instrument revision is stale.
    #[error("instrument revision conflict")]
    RevisionConflict,
    /// A receipt transition was attempted from a terminal or incompatible state.
    #[error("receipt state does not allow this transition")]
    InvalidReceiptState,
    /// The supplied frame ordinal is not the next durable ordinal.
    #[error("receipt frame is out of sequence")]
    FrameOutOfSequence,
    /// A complete receipt must have at least one frame.
    #[error("receipt has no durable frames")]
    ReceiptHasNoFrames,
    /// The synced artifact does not match the exact durable frame content.
    #[error("message artifact does not match durable receipt frames")]
    ArtifactMismatch,
    /// Frame bytes do not match the receipt's protocol and frozen settings.
    #[error("instrument frame bytes are invalid")]
    InvalidFrame,
    /// Intake was attempted for a disabled instrument.
    #[error("instrument listener is disabled")]
    InstrumentDisabled,
    /// Receipt transport or peer does not match the configured connection.
    #[error("receipt transport does not match the instrument connection")]
    TransportMismatch,
    /// TCP receipt source is not configured for the instrument.
    #[error("TCP peer is not allowed for the instrument")]
    UnexpectedTcpPeer,
    /// A watched folder must be an absolute path without control characters.
    #[error("the folder path is not valid")]
    InvalidResultFolder,
    /// Another instrument already watches the folder (F13).
    #[error("this folder is already watched for another instrument")]
    ResultFolderInUse,
    /// A folder pickup for an instrument whose watched folder was cleared.
    #[error("the instrument has no watched folder")]
    ResultFolderNotSet,
    /// A result file name is empty or too long.
    #[error("the file name is not valid")]
    InvalidFileName,
    /// A file over the size limit was offered as received, or one within it
    /// as too large.
    #[error("the file size does not fit this receipt")]
    InvalidFileSize,
    /// The watched-folder file with this name, size and time was received before.
    #[error("this file was already received")]
    FileAlreadyReceived,
    /// A folder pickup from a folder the instrument no longer watches.
    #[error("the instrument's watched folder changed")]
    ResultFolderChanged,
    /// The file was in the folder when it was chosen, so it is not picked up;
    /// it can still be loaded by hand (§6).
    #[error("this file was in the folder when it was chosen")]
    FilePresentAtSelection,
}

impl InterfaceError {
    /// The `CommandError` code the engine reports for this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Store(error) => error.code(),
            Self::InstrumentNotFound | Self::ReceiptNotFound => ErrorCode::NotFound,
            Self::InvalidInstrumentCode
            | Self::InvalidInstrumentName
            | Self::InvalidTcpBindAddress
            | Self::InvalidTcpPort
            | Self::InvalidTcpAllowedPeers
            | Self::InvalidSerialPort
            | Self::InvalidBaudRate
            | Self::InvalidAuConfiguration
            | Self::InvalidFrame
            | Self::InvalidResultFolder
            | Self::InvalidFileName
            | Self::InvalidFileSize => ErrorCode::Validation,
            Self::InstrumentCodeInUse
            | Self::ConnectionInUse
            | Self::RevisionConflict
            | Self::InvalidReceiptState
            | Self::FrameOutOfSequence
            | Self::ReceiptHasNoFrames
            | Self::InstrumentDisabled
            | Self::TransportMismatch
            | Self::UnexpectedTcpPeer
            | Self::ResultFolderInUse
            | Self::ResultFolderNotSet
            | Self::FileAlreadyReceived
            | Self::ResultFolderChanged
            | Self::FilePresentAtSelection => ErrorCode::Conflict,
            Self::ArtifactMismatch => ErrorCode::Internal,
        }
    }

    /// The configuration field at fault, for the UI to highlight. Field paths
    /// follow the camelCase IPC input (`connection.port`).
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        let (field, code, message) = match self {
            Self::InvalidInstrumentCode => (
                "code",
                "invalid_code",
                "Use up to 64 letters, digits, '.', '_' or '-'.",
            ),
            Self::InvalidInstrumentName => (
                "name",
                "invalid_name",
                "Enter a name of up to 128 bytes without control characters.",
            ),
            Self::InvalidTcpBindAddress => (
                "connection.bindAddress",
                "not_lab_address",
                "Use a loopback or private lab network address.",
            ),
            Self::InvalidTcpPort => (
                "connection.port",
                "out_of_range",
                "Use a port from 1 to 65535.",
            ),
            Self::InvalidTcpAllowedPeers => (
                "connection.allowedPeers",
                "invalid_allow_list",
                "List each analyzer address once; every address must be on the lab network.",
            ),
            Self::InvalidSerialPort => (
                "connection.portPath",
                "invalid_port",
                "Enter the serial port name without control characters.",
            ),
            Self::InvalidBaudRate => (
                "connection.baudRate",
                "out_of_range",
                "Use a baud rate from 1 to 4000000.",
            ),
            Self::InvalidAuConfiguration => (
                "connection.au",
                "invalid_au_profile",
                "Check the AU online parameter sheet, field widths, line settings and timers.",
            ),
            Self::Store(_)
            | Self::InstrumentNotFound
            | Self::ReceiptNotFound
            | Self::InstrumentCodeInUse
            | Self::ConnectionInUse
            | Self::RevisionConflict
            | Self::InvalidReceiptState
            | Self::FrameOutOfSequence
            | Self::ReceiptHasNoFrames
            | Self::ArtifactMismatch
            | Self::InvalidFrame
            | Self::InstrumentDisabled
            | Self::TransportMismatch
            | Self::UnexpectedTcpPeer
            | Self::InvalidResultFolder
            | Self::ResultFolderInUse
            | Self::ResultFolderNotSet
            | Self::InvalidFileName
            | Self::InvalidFileSize
            | Self::FileAlreadyReceived
            | Self::ResultFolderChanged
            | Self::FilePresentAtSelection => return Vec::new(),
        };
        vec![FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
        }]
    }
}

impl From<rusqlite::Error> for InterfaceError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}
