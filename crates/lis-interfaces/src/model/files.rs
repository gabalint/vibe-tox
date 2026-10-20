//! Result files exported by instrument software (result-files brief F1, F13, F16).

use lis_core::{InstrumentId, Phi, UserId, UtcTimestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ReceiptId;

/// How a result file reached the LIS.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "ResultFileOrigin")]
pub enum FileOrigin {
    /// A person chose the file (Load results file).
    Upload,
    /// The LIS picked it up from the instrument's watched folder.
    Folder,
}

impl FileOrigin {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Folder => "folder",
        }
    }

    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "upload" => Ok(Self::Upload),
            "folder" => Ok(Self::Folder),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

impl rusqlite::types::FromSql for FileOrigin {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Self::parse(value.as_str()?).map_err(|_| rusqlite::types::FromSqlError::InvalidType)
    }
}

/// How a result file reached the LIS, with what its receipt rechecks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileSource {
    /// A person chose the file (Load results file).
    Upload,
    /// The poller listed the file in this watched folder. The receipt is
    /// refused unless the instrument still watches that folder.
    Folder(String),
}

impl FileSource {
    /// The origin the receipt records.
    #[must_use]
    pub const fn origin(&self) -> FileOrigin {
        match self {
            Self::Upload => FileOrigin::Upload,
            Self::Folder(_) => FileOrigin::Folder,
        }
    }
}

/// What the LIS knows about a result file before it reads it. The name, size
/// and modification time also key the ledger of files already received.
#[derive(Clone, Debug)]
pub struct FileFacts {
    /// The result-file instrument the file is attributed to.
    pub instrument_id: InstrumentId,
    /// Manual load, or folder pickup from a named folder.
    pub source: FileSource,
    /// The file's name without its folder. Stored, never logged.
    pub file_name: Phi<String>,
    /// Size in bytes.
    pub byte_len: u64,
    /// The file system's modification time.
    pub modified_at: UtcTimestamp,
}

/// A result file whose exact bytes the caller has already put in the artifact
/// store (REQ-INT-009: durable before any parsing).
#[derive(Clone, Debug)]
pub struct ReceiveResultFileInput {
    /// The file.
    pub file: FileFacts,
    /// SHA-256 of the bytes, lowercase hex.
    pub sha256: String,
    /// The artifact store's digest of the same bytes.
    pub artifact_digest: String,
}

/// Input for setting or clearing a result-file instrument's watched folder.
#[derive(Clone, Debug)]
pub struct SetResultFolder {
    /// The result-file instrument.
    pub instrument_id: InstrumentId,
    /// Revision observed by the caller.
    pub expected_revision: i64,
    /// The folder to watch; `None` clears it.
    pub folder: Option<WatchedFolder>,
}

/// A folder chosen for watching, with the files already in it (brief §6,
/// "Present at selection"): the poller never picks those up.
#[derive(Clone, Debug)]
pub struct WatchedFolder {
    /// The canonical absolute folder path.
    pub path: String,
    /// The candidate files the folder held when it was chosen.
    pub present: Vec<PresentFile>,
}

/// A file found in a folder when it was chosen.
#[derive(Clone, Debug)]
pub struct PresentFile {
    /// The file's name without its folder. Stored, never logged.
    pub file_name: Phi<String>,
    /// Size in bytes.
    pub byte_len: u64,
    /// The file system's modification time.
    pub modified_at: UtcTimestamp,
}

/// The file facts of a result-file receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReceiptFileView {
    /// The receipt.
    pub receipt_id: ReceiptId,
    /// Manual load or folder pickup.
    pub origin: FileOrigin,
    /// The file's name as received.
    pub file_name: Phi<String>,
    /// Size in bytes.
    #[ts(type = "number")]
    pub byte_len: u64,
    /// The file system's modification time.
    pub modified_at: UtcTimestamp,
    /// The user who loaded it by hand; absent for a folder pickup.
    pub loaded_by: Option<UserId>,
}
