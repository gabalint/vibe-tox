//! Connection settings, messages and their read models.

use lis_core::{Id, text_enum};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use super::intake_model::{IntakeSettings, MovedTo, MovedToChoice, UnknownPatient};
pub use super::mllp_model::{
    AckCode, AllowedPartner, CharacterSet, MllpAcknowledgement, MllpListener, MllpPartner,
};

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
        /// The issued report PDF, sent when a report revision is released.
        ReportPdf = "report_pdf",
        /// An HL7 v2.5.1 ORU^R01 result message with the partner's codes, sent when a
        /// report revision is released.
        Hl7Oru = "hl7_oru",
        /// An HL7 v2.5.1 order message (OML^O21, or ORM^O01) received from a clinic's
        /// EMR; it becomes an order the laboratory accepts or rejects.
        Hl7Orders = "hl7_orders",
    }
}

impl PartnerMessageType {
    /// Whether the LIS receives this type, rather than sends it.
    #[must_use]
    pub const fn incoming(self) -> bool {
        matches!(self, Self::Hl7Orders)
    }
}

text_enum! {
    /// How messages travel.
    #[ts(export)]
    PartnerProtocol {
        /// A folder on this computer or on the lab network.
        Folder = "folder",
        /// The partner's SFTP server, over the internet, encrypted.
        Sftp = "sftp",
        /// HL7 over MLLP to the partner's interface on the lab network; the LIS connects.
        MllpConnect = "mllp_connect",
        /// HL7 over MLLP from the partner, on the lab network only; the LIS listens.
        MllpListen = "mllp_listen",
    }
}

text_enum! {
    /// How the LIS signs in to an SFTP server.
    #[ts(export)]
    SftpAuth {
        /// The LIS's own SSH key, whose public key the partner's IT installs.
        Key = "key",
        /// A password, kept sealed with the installation key.
        Password = "password",
    }
}

/// Where an SFTP connection writes. The server's key must match `fingerprint`, or the LIS
/// does not connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SftpSettings {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SftpAuth,
    /// `SHA256:` and the base64 digest of the server's key, from the partner's IT.
    pub fingerprint: String,
    /// The folder on the server the files go to, such as `/inbound/results`.
    pub remote_folder: String,
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
        /// A newer revision or a withdrawal replaced it before it was written; never written.
        Superseded = "superseded",
    }
}

text_enum! {
    /// Why a delivery exists.
    #[ts(export)]
    DeliveryReason {
        /// The connection's rules matched.
        Routed = "routed",
        /// The connection had an earlier revision of the report, so it gets this one first,
        /// whatever its rules (REQ-RPT-013).
        FollowUp = "follow_up",
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
        /// A released report revision.
        ReportRevision = "report_revision",
        /// A report's withdrawal notice.
        ReportWithdrawal = "report_withdrawal",
        /// A synthetic test message, with no patient data.
        Test = "test",
        /// An order message received from a clinic's EMR.
        Order = "order",
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
        /// A new version of its code mappings was approved.
        MappingsApproved = "mappings_approved",
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
    /// The server, for the SFTP protocol; `None` for a folder.
    #[serde(default)]
    pub sftp: Option<SftpSettings>,
    /// How incoming orders are read and accepted; `None` for an outgoing type.
    #[serde(default)]
    pub intake: Option<IntakeSettings>,
    /// The partner's interface, for MLLP – LIS connects; `None` otherwise.
    #[serde(default)]
    pub mllp: Option<MllpPartner>,
    /// Where the LIS listens, for MLLP – LIS listens; `None` otherwise.
    #[serde(default)]
    pub mllp_listener: Option<MllpListener>,
}

impl PartnerConnectionSettings {
    /// Whether messages travel over MLLP, either way: such a connection has no folder.
    #[must_use]
    pub const fn over_mllp(&self) -> bool {
        self.mllp.is_some() || self.mllp_listener.is_some()
    }
}

/// Input of [`create_partner_connection`](crate::create_partner_connection).
#[derive(Debug, Clone)]
pub struct CreatePartnerConnection {
    pub settings: PartnerConnectionSettings,
    /// The canonical folder the engine checked; empty for SFTP, whose folder is remote.
    pub folder: String,
    /// For incoming orders over a folder: the canonical Processed and Failed folders.
    pub moved_to: MovedToChoice,
    /// The SFTP password, sealed by the engine with the installation key.
    pub sealed_password: Option<Vec<u8>>,
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
    /// Newly chosen Processed and Failed folders; each `None` keeps the current one.
    pub moved_to: MovedToChoice,
    /// A new sealed SFTP password; `None` keeps the current one.
    pub sealed_password: Option<Vec<u8>>,
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
    pub source: MessageSource,
    /// The source record: a billing handoff, a report revision, or the withdrawn report.
    pub source_id: String,
    /// The handoff number, or the report's revision number.
    pub source_number: i64,
    /// For a report's revision or withdrawal, the report it belongs to.
    pub report: Option<ReportRef>,
    pub artifact_sha256: String,
    pub facts: RoutingFacts,
}

/// A report whose revisions and withdrawal are messages. No patient data.
#[derive(Debug, Clone)]
pub struct ReportRef {
    pub report_id: String,
    /// The report number: the accession number and the specimen type code.
    pub report_number: String,
}

/// What routing did with a new message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// No message was kept: no connection carries the message type or, for a
    /// withdrawal notice, none had the report.
    NoConnection,
    /// Kept, with a delivery to each of `deliveries` connections; zero means
    /// it waits in Unrouted messages.
    Kept {
        message_id: PartnerMessageId,
        deliveries: usize,
    },
}
