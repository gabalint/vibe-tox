//! A connection's read models: its settings, destination check, how it is working, and
//! its history.

use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

use super::contract_model::{ContractSummary, PartnerContract};
use super::model::{ConnectionChange, PartnerConnectionId, PartnerConnectionSettings};
use crate::model::ChangeStamp;

/// The current destination check of a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerVerification {
    pub method: String,
    pub verified: ChangeStamp,
    pub self_approval: bool,
}

/// Whether a connection works now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ConnectionStatus {
    /// Started, and its latest attempt worked or none was made.
    Active,
    /// Stopped by a person.
    Stopped,
    /// Started, but its latest attempt failed.
    Error,
}

/// The latest attempt to write on a connection, or to read an incoming one's folder or server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LastAttempt {
    pub at: UtcTimestamp,
    /// `None` when it worked.
    pub error_code: Option<String>,
}

/// An MLLP listener as it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListenerView {
    /// It listens; otherwise `problem` says why not.
    pub listening: bool,
    /// Such as `port_in_use` or `address_not_on_this_computer`; PHI-free.
    pub problem: Option<String>,
    /// Since when it listens, or since the problem.
    pub since: UtcTimestamp,
}

/// A connection as configured, with its check and how it is working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerConnection {
    pub id: PartnerConnectionId,
    pub settings: PartnerConnectionSettings,
    /// The local folder, or the remote folder on the SFTP server: where files are written,
    /// or for incoming orders where they are read.
    pub folder: String,
    /// For incoming orders: where read files move to, local or on the server.
    pub processed_folder: Option<String>,
    pub failed_folder: Option<String>,
    /// For SFTP password sign-in: whether a password is kept. The password itself never
    /// leaves the engine.
    pub has_password: bool,
    pub active: bool,
    #[ts(type = "number")]
    pub revision: i64,
    pub status: ConnectionStatus,
    /// `None` until a check covers the current destination.
    pub verification: Option<PartnerVerification>,
    pub last_attempt: Option<LastAttempt>,
    /// Messages not written yet.
    pub waiting: u32,
    /// Messages whose attempts failed, retrying or given up.
    pub failing: u32,
    /// Messages sent over MLLP without an acknowledgement yet; they are sent again.
    pub awaiting_ack: u32,
    /// For MLLP – LIS listens: whether the listener listens now, as the engine sees it;
    /// `None` while it does not try (stopped, not checked) and for other connections.
    pub listener: Option<ListenerView>,
    /// Its interface contract version and whether its acceptance tests pass for it as it is
    /// now; patient data waits until both hold.
    pub contract: ContractSummary,
    pub last_change: ChangeStamp,
    /// Commands that apply to the connection as it is now.
    pub allowed_actions: Vec<String>,
}

/// One entry of a connection's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionChangeView {
    #[ts(type = "number")]
    pub revision: i64,
    pub change: ConnectionChange,
    pub changed: ChangeStamp,
    pub reason: Option<String>,
    /// The settings that differ from the revision before, by name
    /// (`name`, `folder`, `rules`, …).
    pub fields: Vec<String>,
}

/// A connection with its interface contract and its history, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerConnectionDetail {
    pub connection: PartnerConnection,
    pub contract: PartnerContract,
    pub history: Vec<ConnectionChangeView>,
}
