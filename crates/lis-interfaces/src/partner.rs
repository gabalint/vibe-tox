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
//! its folder, its interface contract is approved with acceptance tests passed for
//! it as it is now (REQ-INT-001), and its schedule allows, and records each attempt
//! here. Analyzers are configured separately, as instruments.

mod acceptance;
mod acceptance_cases;
mod connection_columns;
mod connection_views;
mod connections;
mod contract_model;
mod contracts;
mod delivery;
mod error;
pub mod hl7;
pub mod hl7_order;
mod hl7_read;
mod intake_model;
mod intake_settings;
mod intake_sources;
mod mapping_model;
mod mapping_repo;
mod mappings;
mod message_filter;
mod message_queries;
mod message_repo;
mod messages;
pub mod mllp;
pub mod mllp_answer;
mod mllp_listener_settings;
mod mllp_model;
mod mllp_rejections;
mod mllp_settings;
mod model;
mod queries;
mod queue;
mod received;
mod received_queries;
mod rejections;
mod repo;
mod rules;
mod settings;
mod sftp_identity;
mod sftp_settings;
mod unrouted_queries;

pub use acceptance::{current_acceptance_key, record_acceptance_run};
pub use acceptance_cases::outgoing_cases;
pub use connection_views::{
    ConnectionChangeView, ConnectionStatus, LastAttempt, ListenerView, PartnerConnection,
    PartnerConnectionDetail, PartnerVerification,
};
pub use connections::{
    create_partner_connection, record_partner_destination_check, set_partner_connection_active,
    update_partner_connection,
};
pub use contract_model::{
    AcceptanceCase, AcceptanceRun, AcceptanceState, ApprovePartnerContract, ContractDraft,
    ContractEdge, ContractSummary, ContractVersion, DraftPartnerContract, InterfaceContract,
    PartnerContract,
};
pub use contracts::{approve_partner_contract, draft_partner_contract, partner_contract};
pub use delivery::{
    AttemptOutcome, ConnectionTarget, Destination, DueDelivery, MAX_ATTEMPTS,
    due_partner_deliveries, partner_connection_folder, record_delivery_attempt,
    record_delivery_payload, record_partner_test_message,
};
pub use error::PartnerError;
pub use intake_sources::{IntakeSource, intake_source, intake_sources};
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
pub use mllp_listener_settings::valid_mllp_listener;
pub use mllp_rejections::record_rejected_partners;
pub use mllp_settings::valid_mllp_partner;
pub use model::{
    AckCode, AllowedPartner, CharacterSet, ConnectionChange, CreatePartnerConnection,
    DeliveryReason, DeliveryState, IntakeSettings, MessageSource, MllpAcknowledgement,
    MllpListener, MllpPartner, MovedTo, MovedToChoice, NewPartnerMessage, PartnerConnectionId,
    PartnerConnectionSettings, PartnerDeliveryId, PartnerMessageId, PartnerMessageType,
    PartnerProtocol, RecordPartnerDestinationCheck, ReportRef, Routed, RoutingFacts, RoutingRule,
    RoutingValue, RuleCondition, RuleField, Schedule, SetPartnerConnectionActive, SftpAuth,
    SftpSettings, UnknownPatient, UnroutedResolution, UpdatePartnerConnection,
};
pub use queries::{carries, get_partner_connection, list_partner_connections};
pub use queue::{FailedItem, UnroutedItem, failed_queue, unrouted_queue};
pub use received::{
    EarlierRead, NewReceived, PartnerReceivedId, ReceivedOutcome, ReceivedState, WaitingMessage,
    control_id_seen, earlier_read, received_ack, record_moved, record_received, resolve_waiting,
    waiting_messages,
};
pub use rejections::{
    RejectReceivedMessage, RejectionResolution, ResolvePartnerRejection, reject_received_message,
    resolve_partner_rejection,
};
pub use rules::{rules_match, schedule_due};
pub use sftp_identity::{create_sftp_identity, sealed_sftp_key, sftp_public_key};
pub use sftp_settings::valid_sftp_server;
pub use unrouted_queries::{ClosestRule, UnroutedMessage, list_unrouted_partner_messages};
