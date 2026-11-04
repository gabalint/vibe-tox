//! Connection settings, messages and their read models.

use lis_core::{Id, UtcTimestamp, text_enum};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ChangeStamp;

/// Marker for one connection to an outside system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerConnectionMarker {}
/// One connection to an outside system.
pub type PartnerConnectionId = Id<PartnerConnectionMarker>;

/// Marker for one outgoing message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerMessageMarker {}
/// One outgoing message, whatever the number of connections it goes to.
pub type PartnerMessageId = Id<PartnerMessageMarker>;

/// Marker for one message's delivery to one connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerDeliveryMarker {}
/// One message's delivery to one connection.
pub type PartnerDeliveryId = Id<PartnerDeliveryMarker>;

text_enum! {
    /// What a connection carries. The type fixes the direction and when a
    /// message is made.
    #[ts(export)]
    PartnerMessageType {
        /// The billing handoff file, sent when a handoff is created.
        BillingJson = "billing_json",
    }
}

text_enum! {
    /// How messages travel.
    #[ts(export)]
    PartnerProtocol {
        /// A folder on this computer or on the lab network.
        Folder = "folder",
    }
}

text_enum! {
    /// The LIS value a routing rule looks at.
    #[ts(export)]
    RuleField {
        /// The ordering client's code.
        Client = "client",
        /// The ordering practitioner's NPI.
        Practitioner = "practitioner",
        /// The client location's code.
        Location = "location",
        /// The ordered test's code.
        Test = "test",
    }
}

text_enum! {
    /// How a routing rule compares.
    #[ts(export)]
    RuleCondition {
        /// The value is exactly the rule's value.
        Equals = "equals",
        /// The value is one of the rule's comma-separated values.
        OneOf = "one_of",
        /// The value starts with the rule's value.
        StartsWith = "starts_with",
    }
}

text_enum! {
    /// Where a delivery stands.
    #[ts(export)]
    DeliveryState {
        /// Not written yet: waiting for the connection, its schedule or a retry.
        Waiting = "waiting",
        /// Written to the folder. The folder gives no confirmation of use.
        Written = "written",
        /// Every attempt failed; it waits for someone to send it again.
        Failed = "failed",
    }
}

text_enum! {
    /// Why a delivery exists.
    #[ts(export)]
    DeliveryReason {
        /// The connection's rules matched.
        Routed = "routed",
        /// Sent once to the connection from Unrouted messages.
        SentOnce = "sent_once",
        /// A synthetic test message.
        Test = "test",
    }
}

text_enum! {
    /// What a message was made from.
    #[ts(export)]
    MessageSource {
        /// A billing handoff.
        BillingHandoff = "billing_handoff",
        /// A synthetic test message, with no patient data.
        Test = "test",
    }
}

text_enum! {
    /// The decision on a message that matched no connection.
    #[ts(export)]
    UnroutedResolution {
        /// Sent once to a chosen connection.
        SentOnce = "sent_once",
        /// Routed by the rules after they changed.
        RoutedAgain = "routed_again",
        /// Delivered outside the LIS, for example by fax.
        DeliveredAnotherWay = "delivered_another_way",
        /// Not delivered, for the reason given.
        Dismissed = "dismissed",
    }
}

text_enum! {
    /// One entry in a connection's history.
    #[ts(export)]
    ConnectionChange {
        /// Added.
        Created = "created",
        /// Settings changed.
        Updated = "updated",
        /// Started: it sends.
        Started = "started",
        /// Stopped: it sends nothing.
        Stopped = "stopped",
        /// Its destination check was recorded.
        Verified = "verified",
    }
}

/// One routing rule: the message's values for `field` must all meet
/// `condition` with `value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RoutingRule {
    pub field: RuleField,
    pub condition: RuleCondition,
    /// For `one_of`, values separated by commas.
    pub value: String,
}

/// When a connection writes its waiting messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum Schedule {
    /// At once.
    Immediately,
    /// At every multiple of `minutes` since midnight UTC.
    EveryMinutes { minutes: u16 },
    /// Daily at `time` (`HH:MM`) in the laboratory's time zone.
    DailyAt { time: String },
}

/// What a person sets on a connection. The folder is chosen in a system
/// dialog and passed separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PartnerConnectionSettings {
    pub name: String,
    /// The outside party: a client or a billing company.
    pub partner: String,
    pub message_type: PartnerMessageType,
    pub protocol: PartnerProtocol,
    pub rules: Vec<RoutingRule>,
    pub schedule: Schedule,
    /// For example `billing-{handoff}_{date}.json`.
    pub file_name_pattern: String,
}

/// Input of [`create_partner_connection`](crate::create_partner_connection).
#[derive(Debug, Clone)]
pub struct CreatePartnerConnection {
    pub settings: PartnerConnectionSettings,
    /// The canonical folder the engine checked.
    pub folder: String,
    /// False for "Save as stopped".
    pub active: bool,
}

/// Input of [`update_partner_connection`](crate::update_partner_connection).
#[derive(Debug, Clone)]
pub struct UpdatePartnerConnection {
    pub connection_id: PartnerConnectionId,
    pub expected_revision: i64,
    pub settings: PartnerConnectionSettings,
    /// A newly chosen canonical folder; `None` keeps the current one.
    pub folder: Option<String>,
    pub reason: String,
}

/// Input of `set_partner_connection_active`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetPartnerConnectionActive {
    pub connection_id: PartnerConnectionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub active: bool,
    pub reason: String,
}

/// Input of `record_partner_destination_check`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordPartnerDestinationCheck {
    pub connection_id: PartnerConnectionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// How the destination was confirmed, for example a call-back to the
    /// partner's IT.
    pub method: String,
    /// The person who chose the folder confirms checking it themselves.
    #[serde(default)]
    pub self_approval: bool,
}

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

/// The latest attempt to write on a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LastAttempt {
    pub at: UtcTimestamp,
    /// `None` when it worked.
    pub error_code: Option<String>,
}

/// A connection as configured, with its check and how it is working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerConnection {
    pub id: PartnerConnectionId,
    pub settings: PartnerConnectionSettings,
    pub folder: String,
    pub active: bool,
    #[ts(type = "number")]
    pub revision: i64,
    pub status: ConnectionStatus,
    /// `None` until a check covers the current folder.
    pub verification: Option<PartnerVerification>,
    pub last_attempt: Option<LastAttempt>,
    /// Messages not written yet.
    pub waiting: u32,
    /// Messages whose attempts failed, retrying or given up.
    pub failing: u32,
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

/// A connection with its history, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerConnectionDetail {
    pub connection: PartnerConnection,
    pub history: Vec<ConnectionChangeView>,
}

/// One LIS value a rule can match, with its display label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutingValue {
    pub value: String,
    pub label: String,
}

/// The values of a message that routing rules look at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutingFacts {
    pub clients: Vec<RoutingValue>,
    pub practitioners: Vec<RoutingValue>,
    pub locations: Vec<RoutingValue>,
    pub tests: Vec<RoutingValue>,
}

/// A message to route, made in the transaction that creates its source.
#[derive(Debug, Clone)]
pub struct NewPartnerMessage {
    pub message_type: PartnerMessageType,
    /// The source record (a billing handoff).
    pub source_id: String,
    pub source_number: i64,
    pub artifact_sha256: String,
    pub facts: RoutingFacts,
}

/// What routing did with a new message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// No connection carries the message type, so no message was kept.
    NoConnection,
    /// Kept, with a delivery to each of `deliveries` connections; zero means
    /// it waits in Unrouted messages.
    Kept {
        message_id: PartnerMessageId,
        deliveries: usize,
    },
}
