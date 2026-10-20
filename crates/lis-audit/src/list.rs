//! Paged reading of one stream for the audit viewer (REQ-OPS-001). The
//! caller authorizes the read and records it as an `access` event.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, params};

use crate::model::{AuditError, SELF_APPROVAL_DETAIL, Stream};

/// Which events of a stream to read, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFilter<'a> {
    /// The stream.
    pub stream: Stream,
    /// Only events with a lower sequence number (the next page).
    pub before_seq: Option<i64>,
    /// At most this many events.
    pub limit: u32,
    /// Only this action.
    pub action: Option<&'a str>,
    /// Only this record type.
    pub record_type: Option<&'a str>,
    /// Only this record ID.
    pub record_id: Option<&'a str>,
    /// Only self-approvals ([`crate::AuditEvent::self_approval`]).
    pub self_approvals_only: bool,
}

/// One stored event, without its chain and context columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    /// Its stream.
    pub stream: Stream,
    /// Its sequence number in the stream.
    pub seq: i64,
    /// When the action happened.
    pub occurred_at: UtcTimestamp,
    /// When the event was appended.
    pub recorded_at: UtcTimestamp,
    /// `user`, `instrument`, `partner` or `system`.
    pub principal_kind: String,
    /// User or instrument ID, or system component name.
    pub principal_id: String,
    /// `<module>.<verb>`.
    pub action: String,
    /// `succeeded`, `denied` or `failed`.
    pub outcome: String,
    /// Type of the affected record.
    pub record_type: Option<String>,
    /// ID of the affected record.
    pub record_id: Option<String>,
    /// Why, as entered.
    pub reason: Option<String>,
    /// Capabilities that permitted the action.
    pub authority: Option<String>,
    /// Shared with the diagnostic log line.
    pub correlation_id: String,
}

/// The events of `filter.stream` that match it, newest first.
pub fn list(conn: &Connection, filter: &EventFilter<'_>) -> Result<Vec<StoredEvent>, AuditError> {
    let stream = filter.stream;
    let events = conn
        .prepare_cached(
            "SELECT seq, occurred_at, recorded_at, principal_kind, principal_id, action, outcome, \
             record_type, record_id, reason, authority, correlation_id FROM audit_events \
             WHERE stream = ?1 AND (?2 IS NULL OR seq < ?2) AND (?3 IS NULL OR action = ?3) \
             AND (?4 IS NULL OR record_type = ?4) AND (?5 IS NULL OR record_id = ?5) \
             AND (NOT ?7 OR json_extract(details, '$.' || ?8) IS 1) \
             ORDER BY seq DESC LIMIT ?6",
        )?
        .query_map(
            params![
                stream.as_str(),
                filter.before_seq,
                filter.action,
                filter.record_type,
                filter.record_id,
                filter.limit,
                filter.self_approvals_only,
                SELF_APPROVAL_DETAIL
            ],
            |row| {
                Ok(StoredEvent {
                    stream,
                    seq: row.get(0)?,
                    occurred_at: row.get(1)?,
                    recorded_at: row.get(2)?,
                    principal_kind: row.get(3)?,
                    principal_id: row.get(4)?,
                    action: row.get(5)?,
                    outcome: row.get(6)?,
                    record_type: row.get(7)?,
                    record_id: row.get(8)?,
                    reason: row.get(9)?,
                    authority: row.get(10)?,
                    correlation_id: row.get(11)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(events)
}

/// Distinct action codes present in `stream`, in code order.
pub fn actions(conn: &Connection, stream: Stream) -> Result<Vec<String>, AuditError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT action FROM audit_events WHERE stream = ?1 ORDER BY action",
        )?
        .query_map([stream.as_str()], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Distinct record types present in `stream`, in code order.
pub fn record_types(conn: &Connection, stream: Stream) -> Result<Vec<String>, AuditError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT record_type FROM audit_events \
             WHERE stream = ?1 AND record_type IS NOT NULL ORDER BY record_type",
        )?
        .query_map([stream.as_str()], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}
