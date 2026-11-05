//! How an HL7 Orders connection reads and accepts incoming order messages (ticket #4555
//! step 3c; REQ-INT-002, REQ-INT-003, REQ-ORD-001).

use lis_core::text_enum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

text_enum! {
    /// What happens to an order whose patient the LIS cannot find by identifier.
    #[ts(export)]
    UnknownPatient {
        /// The message waits; it becomes an order once the patient is registered with the
        /// identifier, or someone rejects it.
        Hold = "hold",
        /// The file moves to Failed and the reason is logged.
        Reject = "reject",
    }
}

/// How incoming orders are read and accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct IntakeSettings {
    /// How often the input folder is read, in seconds.
    pub check_every_seconds: u16,
    /// The sending facility (MSH-4) the partner's messages carry; any other message moves
    /// to Failed.
    pub accept_from: String,
    pub unknown_patient: UnknownPatient,
    /// For SFTP: the folder on the server read files move to; a folder connection's
    /// Processed folder is chosen in the system dialog instead.
    #[serde(default)]
    #[ts(optional)]
    pub remote_processed_folder: Option<String>,
    /// For SFTP: the folder on the server files with errors move to.
    #[serde(default)]
    #[ts(optional)]
    pub remote_failed_folder: Option<String>,
}

/// Where read files move to, in year and month subfolders: canonical local folders, or
/// folders on the SFTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedTo {
    pub processed: String,
    pub failed: String,
}

/// Newly chosen local Processed and Failed folders; each `None` keeps the current one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MovedToChoice {
    pub processed: Option<String>,
    pub failed: Option<String>,
}
