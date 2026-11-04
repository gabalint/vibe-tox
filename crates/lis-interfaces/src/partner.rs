//! Connections to outside systems (Interfaces menu): what each carries, where
//! it writes, which messages go to it, and the log of every message
//! (REQ-INT-001–003, REQ-RPT-003, REQ-RPT-011–013, REQ-BIL-002).
//!
//! A new billing handoff, a released report revision (as a PDF, or as an HL7
//! ORU^R01 with the partner's approved codes) and a report's withdrawal notice
//! each become one message in the transaction that creates them. Routing
//! gives it a delivery to every connection of its type whose rules all match, or
//! a place in Unrouted messages, where a person decides. A report's later
//! revisions and its withdrawal go first to the connections that had an earlier
//! revision (REQ-RPT-013). The engine's delivery poller writes waiting deliveries
//! to their folder once the connection is started, a destination check covers
//! its folder and its schedule allows, and records each attempt here. Analyzers
//! are configured separately, as instruments.

mod connection_columns;
mod connection_views;
mod connections;
mod delivery;
mod error;
pub mod hl7;
pub mod hl7_order;
mod intake_model;
mod intake_settings;
mod intake_sources;
mod mapping_model;
mod mapping_repo;
mod mappings;
mod message_queries;
mod message_repo;
mod messages;
mod model;
mod queries;
mod queue;
mod received;
mod repo;
mod rules;
mod settings;
mod sftp_identity;
mod sftp_settings;
mod unrouted_queries;

pub use connection_views::{
    ConnectionChangeView, ConnectionStatus, LastAttempt, PartnerConnection,
    PartnerConnectionDetail, PartnerVerification,
};
pub use connections::{
    create_partner_connection, record_partner_destination_check, set_partner_connection_active,
    update_partner_connection,
};
pub use delivery::{
    AttemptOutcome, ConnectionTarget, Destination, DueDelivery, MAX_ATTEMPTS,
    due_partner_deliveries, partner_connection_folder, record_delivery_attempt,
    record_delivery_payload, record_partner_test_message,
};
pub use error::PartnerError;
pub use intake_sources::{IntakeSource, intake_sources};
pub use mapping_model::{
    ApprovePartnerMappings, ApprovedMappings, CodeMapping, DiscardPartnerMappingDraft,
    DraftPartnerMapping, LisRecordId, MappingDraft, MappingKind, MappingRow, MappingValues,
    NewMappingDraft, PartnerMappings,
};
pub use mappings::{
    approve_partner_mappings, approved_mappings, discard_partner_mapping_draft,
    draft_partner_mapping, partner_mappings,
};
pub use message_queries::{
    ConnectionRef, ListPartnerMessages, MessageView, PartnerMessagePage, PartnerMessageRow,
    TransportOutcome, WaitReason, list_partner_messages,
};
pub use messages::{
    ResendPartnerDelivery, ResolveUnroutedMessage, UnroutedDecision, resend_partner_delivery,
    resolve_unrouted_message, route_partner_message,
};
pub use model::{
    ConnectionChange, CreatePartnerConnection, DeliveryReason, DeliveryState, IntakeSettings,
    MessageSource, MovedTo, MovedToChoice, NewPartnerMessage, PartnerConnectionId,
    PartnerConnectionSettings, PartnerDeliveryId, PartnerMessageId, PartnerMessageType,
    PartnerProtocol, RecordPartnerDestinationCheck, ReportRef, Routed, RoutingFacts, RoutingRule,
    RoutingValue, RuleCondition, RuleField, Schedule, SetPartnerConnectionActive, SftpAuth,
    SftpSettings, UnknownPatient, UnroutedResolution, UpdatePartnerConnection,
};
pub use queries::{carries, get_partner_connection, list_partner_connections};
pub use queue::{FailedItem, UnroutedItem, failed_queue, unrouted_queue};
pub use received::{
    EarlierRead, NewReceived, PartnerReceivedId, ReceivedOutcome, ReceivedState, WaitingMessage,
    control_id_seen, earlier_read, record_moved, record_received, resolve_waiting,
    waiting_messages,
};
pub use rules::{rules_match, schedule_due};
pub use sftp_identity::{create_sftp_identity, sealed_sftp_key, sftp_public_key};
pub use sftp_settings::valid_sftp_server;
pub use unrouted_queries::{ClosestRule, UnroutedMessage, list_unrouted_partner_messages};
