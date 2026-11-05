//! Paged reading of one stream or of every stream for the audit viewer
//! (REQ-OPS-001). The caller authorizes the read and records it as an `access` event.

use lis_core::UtcTimestamp;
use rusqlite::types::Type;
use rusqlite::{Connection, Row, params};

use crate::model::{AuditError, Outcome, SELF_APPROVAL_DETAIL, Stream};

/// Which events to read, newest first: by sequence number in one stream, by
/// append time across every stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFilter<'a> {
    /// The stream; `None` reads every stream.
    pub stream: Option<Stream>,
    /// Only events after this one in the order read (the next page).
    pub before: Option<EventCursor>,
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
    /// Only events recorded at or after this instant.
    pub recorded_from: Option<UtcTimestamp>,
    /// Only events recorded before this instant.
    pub recorded_before: Option<UtcTimestamp>,
    /// Only events by these principals.
    pub by: Option<ByFilter<'a>>,
    /// Only events with this outcome.
    pub outcome: Option<Outcome>,
}

/// Who acted, for [`EventFilter::by`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByFilter<'a> {
    /// Signed-in users.
    People,
    /// Everyone else: the system, instruments and partners.
    System,
    /// The user with this ID.
    User(&'a str),
}

impl EventFilter<'_> {
    /// Up to `limit` events of `stream`, newest first, with no other filter.
    #[must_use]
    pub fn new(stream: Stream, limit: u32) -> Self {
        Self {
            stream: Some(stream),
            before: None,
            limit,
            action: None,
            record_type: None,
            record_id: None,
            self_approvals_only: false,
            recorded_from: None,
            recorded_before: None,
            by: None,
            outcome: None,
        }
    }
}

/// The last event of a page. Sequence numbers are per stream, so across streams
/// the order is append time, then stream, then sequence number: unique and
/// unchanged by later appends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCursor {
    /// When the event was appended.
    pub recorded_at: UtcTimestamp,
    /// Its stream.
    pub stream: Stream,
    /// Its sequence number in the stream.
    pub seq: i64,
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

/// The columns of [`StoredEvent`], in its field order.
const COLUMNS: &str = "stream, seq, occurred_at, recorded_at, principal_kind, principal_id, action, \
     outcome, record_type, record_id, reason, authority, correlation_id";

/// The filters besides the stream and the cursor; `?13` is whether a person acted.
const FILTERS: &str = "(?5 IS NULL OR action = ?5) AND (?6 IS NULL OR record_type = ?6) \
     AND (?7 IS NULL OR record_id = ?7) AND (NOT ?8 OR json_extract(details, '$.' || ?9) IS 1) \
     AND (?11 IS NULL OR recorded_at >= ?11) AND (?12 IS NULL OR recorded_at < ?12) \
     AND (?13 IS NULL OR (principal_kind = 'user') = ?13) \
     AND (?14 IS NULL OR principal_id = ?14) AND (?15 IS NULL OR outcome = ?15)";

/// Sorts after every stored timestamp (a digit first) and stream code (a lowercase letter
/// first), so the first page needs no `IS NULL` branch and the cursor stays an index range.
const AFTER_EVERY: &str = "~";

/// The events that match `filter`, newest first.
pub fn list(conn: &Connection, filter: &EventFilter<'_>) -> Result<Vec<StoredEvent>, AuditError> {
    // One stream reads its primary key by sequence number; every stream reads
    // `audit_events_by_time` (migration 4) by append time.
    let (cursor, order) = if filter.stream.is_some() {
        ("stream = ?1 AND seq < ?4", "seq DESC")
    } else {
        (
            "(recorded_at, stream, seq) < (?2, ?3, ?4)",
            "recorded_at DESC, stream DESC, seq DESC",
        )
    };
    let (before_recorded_at, before_stream, before_seq) =
        filter
            .before
            .map_or((AFTER_EVERY.to_owned(), AFTER_EVERY, i64::MAX), |before| {
                (
                    before.recorded_at.to_string(),
                    before.stream.as_str(),
                    before.seq,
                )
            });
    let events = conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM audit_events WHERE {cursor} AND {FILTERS} \
             ORDER BY {order} LIMIT ?10"
        ))?
        .query_map(
            params![
                filter.stream.map(Stream::as_str),
                before_recorded_at,
                before_stream,
                before_seq,
                filter.action,
                filter.record_type,
                filter.record_id,
                filter.self_approvals_only,
                SELF_APPROVAL_DETAIL,
                filter.limit,
                filter.recorded_from,
                filter.recorded_before,
                filter.by.map(|by| by != ByFilter::System),
                match filter.by {
                    Some(ByFilter::User(id)) => Some(id),
                    _ => None,
                },
                filter.outcome.map(Outcome::as_str),
            ],
            stored_event,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(events)
}

fn stored_event(row: &Row<'_>) -> rusqlite::Result<StoredEvent> {
    let stream: String = row.get(0)?;
    Ok(StoredEvent {
        stream: Stream::parse(&stream).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, "unknown stream".into())
        })?,
        seq: row.get(1)?,
        occurred_at: row.get(2)?,
        recorded_at: row.get(3)?,
        principal_kind: row.get(4)?,
        principal_id: row.get(5)?,
        action: row.get(6)?,
        outcome: row.get(7)?,
        record_type: row.get(8)?,
        record_id: row.get(9)?,
        reason: row.get(10)?,
        authority: row.get(11)?,
        correlation_id: row.get(12)?,
    })
}

/// Distinct action codes present in `stream` (`None`: every stream), in code order.
pub fn actions(conn: &Connection, stream: Option<Stream>) -> Result<Vec<String>, AuditError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT action FROM audit_events WHERE ?1 IS NULL OR stream = ?1 \
             ORDER BY action",
        )?
        .query_map([stream.map(Stream::as_str)], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Distinct record types present in `stream` (`None`: every stream), in code order.
pub fn record_types(conn: &Connection, stream: Option<Stream>) -> Result<Vec<String>, AuditError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT record_type FROM audit_events \
             WHERE (?1 IS NULL OR stream = ?1) AND record_type IS NOT NULL ORDER BY record_type",
        )?
        .query_map([stream.map(Stream::as_str)], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}
