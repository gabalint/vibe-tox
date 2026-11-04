//! Connections to outside systems (Interfaces menu): what each carries, where
//! it writes, which messages go to it, and the log of every message
//! (REQ-INT-001–003, REQ-RPT-003, REQ-RPT-011–013, REQ-BIL-002).
//!
//! A new billing handoff becomes one message in the transaction that creates
//! it. Routing gives it a delivery to every connection of its type whose rules
//! all match, or a place in Unrouted messages, where a person decides. The
//! engine's delivery poller writes waiting deliveries to their folder once the
//! connection is started, a destination check covers its folder and its
//! schedule allows, and records each attempt here. Analyzers are configured
//! separately, as instruments.

mod connections;
mod delivery;
mod error;
mod message_queries;
mod message_repo;
mod messages;
mod model;
mod queries;
mod queue;
mod repo;
mod rules;
mod settings;

pub use connections::{
    create_partner_connection, record_partner_destination_check, set_partner_connection_active,
    update_partner_connection,
};
pub use delivery::{
    AttemptOutcome, DueDelivery, MAX_ATTEMPTS, due_partner_deliveries, partner_connection_folder,
    record_delivery_attempt, record_partner_test_message,
};
pub use error::PartnerError;
pub use message_queries::{
    ClosestRule, ConnectionRef, ListPartnerMessages, MessageView, PartnerMessagePage,
    PartnerMessageRow, TransportOutcome, UnroutedMessage, WaitReason, list_partner_messages,
    list_unrouted_partner_messages,
};
pub use messages::{
    ResendPartnerDelivery, ResolveUnroutedMessage, UnroutedDecision, resend_partner_delivery,
    resolve_unrouted_message, route_partner_message,
};
pub use model::{
    ConnectionChange, ConnectionChangeView, ConnectionStatus, CreatePartnerConnection,
    DeliveryReason, DeliveryState, LastAttempt, MessageSource, NewPartnerMessage,
    PartnerConnection, PartnerConnectionDetail, PartnerConnectionId, PartnerConnectionSettings,
    PartnerDeliveryId, PartnerMessageId, PartnerMessageType, PartnerProtocol, PartnerVerification,
    RecordPartnerDestinationCheck, Routed, RoutingFacts, RoutingRule, RoutingValue, RuleCondition,
    RuleField, Schedule, SetPartnerConnectionActive, UnroutedResolution, UpdatePartnerConnection,
};
pub use queries::{carries, get_partner_connection, list_partner_connections};
pub use queue::{FailedItem, UnroutedItem, failed_queue, unrouted_queue};
pub use rules::{rules_match, schedule_due};
pub use settings::file_name;
