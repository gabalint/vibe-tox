//! Unrouted messages waiting for a decision (REQ-RPT-003), with the rule that
//! kept each from its closest connection.

use lis_core::UtcTimestamp;
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use super::error::PartnerError;
use super::message_queries::ConnectionRef;
use super::message_repo::{MessageRow, open_unrouted};
use super::model::{
    MessageSource, PartnerMessageId, PartnerMessageType, RoutingFacts, RoutingRule,
};
use super::repo::connections;
use super::rules::unmet;

const UNROUTED_LIMIT: u32 = 500;

/// The rule that kept a message from its closest connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClosestRule {
    pub connection: ConnectionRef,
    pub rule: RoutingRule,
}

/// A message waiting in Unrouted messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UnroutedMessage {
    pub message_id: PartnerMessageId,
    pub created_at: UtcTimestamp,
    pub message_type: PartnerMessageType,
    pub source: MessageSource,
    pub source_id: Option<String>,
    #[ts(type = "number | null")]
    pub source_number: Option<i64>,
    /// The report number, for a report's revision.
    pub source_label: Option<String>,
    pub facts: RoutingFacts,
    /// Connections of the type its rules were checked against when it was made.
    #[ts(type = "number")]
    pub checked_against: i64,
    /// The connection now missing the fewest rules, and its first unmet rule.
    pub closest: Option<ClosestRule>,
    /// Connections that carry its type now, for Send once.
    pub connections: Vec<ConnectionRef>,
    /// `resolve_unrouted_message` while it waits.
    pub allowed_actions: Vec<String>,
}

/// Messages waiting for a decision, oldest first. A message a newer revision of
/// its report replaced no longer waits.
pub fn list_unrouted_partner_messages(
    conn: &Connection,
) -> Result<Vec<UnroutedMessage>, PartnerError> {
    open_unrouted(conn, UNROUTED_LIMIT)?
        .into_iter()
        .map(|message| unrouted_view(conn, message))
        .collect()
}

fn unrouted_view(conn: &Connection, message: MessageRow) -> Result<UnroutedMessage, PartnerError> {
    let candidates = connections(conn, Some(message.message_type))?;
    let closest = candidates
        .iter()
        .filter_map(|connection| {
            let mut missing = unmet(&connection.settings.rules, &message.facts);
            let first = missing.next()?.clone();
            Some((missing.count(), connection, first))
        })
        .min_by_key(|(more, _, _)| *more)
        .map(|(_, connection, rule)| ClosestRule {
            connection: ConnectionRef {
                id: connection.id,
                name: connection.settings.name.clone(),
                protocol: connection.settings.protocol,
            },
            rule,
        });
    Ok(UnroutedMessage {
        message_id: message.id,
        created_at: message.created_at,
        message_type: message.message_type,
        source: message.source,
        source_id: message.source_id,
        source_number: message.source_number,
        source_label: message.source_label,
        facts: message.facts,
        checked_against: message.checked_against,
        closest,
        connections: candidates
            .into_iter()
            .map(|connection| ConnectionRef {
                id: connection.id,
                name: connection.settings.name,
                protocol: connection.settings.protocol,
            })
            .collect(),
        allowed_actions: vec!["resolve_unrouted_message".to_owned()],
    })
}
