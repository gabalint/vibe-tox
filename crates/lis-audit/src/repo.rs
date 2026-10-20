//! SQL for `audit_events`.

use std::collections::BTreeSet;

use lis_core::{Actor, Capability, Principal, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::chain::{COLUMNS, EventRow, GENESIS_HASH, HASH_ALG};
use crate::model::{
    Attribution, AuditError, AuditEvent, ChainLink, Stream, details_json, is_valid_action,
};

/// Appends `event` to its stream inside the caller's transaction.
///
/// Computes the next sequence number, links to the stream head and stores the
/// `sha256-v1` hash. Run it in the same `BEGIN IMMEDIATE` transaction as the
/// change it records: if the append fails, the change must not commit.
pub fn append(
    tx: &Transaction<'_>,
    attribution: &Attribution<'_>,
    event: &AuditEvent,
) -> Result<ChainLink, AuditError> {
    if !is_valid_action(event.action) {
        return Err(AuditError::InvalidAction);
    }
    let details = details_json(&event.details)?;
    let (seq, prev_hash) = match head(tx, event.stream)? {
        Some(head) => (head.seq + 1, head.hash),
        None => (1, GENESIS_HASH.to_owned()),
    };
    let (principal_kind, principal_id, session_id, workstation_id) =
        principal_columns(attribution.principal);
    let row = EventRow {
        hash_alg: HASH_ALG.to_owned(),
        stream: event.stream.as_str().to_owned(),
        seq,
        prev_hash,
        occurred_at: attribution.occurred_at.to_string(),
        recorded_at: attribution.recorded_at.to_string(),
        principal_kind: principal_kind.to_owned(),
        principal_id,
        session_id,
        workstation_id,
        action: event.action.to_owned(),
        outcome: event.outcome.as_str().to_owned(),
        record_type: event.record_type.map(str::to_owned),
        record_id: event.record_id.clone(),
        old_revision: event.old_revision,
        new_revision: event.new_revision,
        reason: event.reason.clone(),
        authority: authority_text(attribution.authority),
        correlation_id: attribution.correlation_id.to_string(),
        details,
    };
    let hash = row.compute_hash();
    insert(tx, &row, &hash)?;
    Ok(ChainLink {
        stream: event.stream,
        seq,
        hash,
    })
}

/// The head of every stream, in [`Stream::ALL`] order (NFR-18, REQ-OPS-007).
///
/// An empty stream's head is sequence 0 with [`GENESIS_HASH`], the value
/// its first event will link to. Backup and export manifests record these.
pub fn heads(conn: &Connection) -> Result<Vec<ChainLink>, AuditError> {
    let mut heads = Vec::with_capacity(Stream::ALL.len());
    for stream in Stream::ALL {
        heads.push(head(conn, stream)?.unwrap_or_else(|| ChainLink {
            stream,
            seq: 0,
            hash: GENESIS_HASH.to_owned(),
        }));
    }
    Ok(heads)
}

/// When the event `(stream, seq)` was appended; `None` when it does not exist.
/// Anchor lag is measured from the oldest event no exported seal covers.
pub fn event_recorded_at(
    conn: &Connection,
    stream: Stream,
    seq: i64,
) -> Result<Option<UtcTimestamp>, AuditError> {
    let recorded_at = conn
        .prepare_cached("SELECT recorded_at FROM audit_events WHERE stream = ?1 AND seq = ?2")?
        .query_row(params![stream.as_str(), seq], |row| row.get(0))
        .optional()?;
    Ok(recorded_at)
}

/// The stored hash of the event `(stream, seq)`, if it exists.
pub(crate) fn stored_hash(
    conn: &Connection,
    stream: Stream,
    seq: i64,
) -> rusqlite::Result<Option<String>> {
    conn.prepare_cached("SELECT hash FROM audit_events WHERE stream = ?1 AND seq = ?2")?
        .query_row(params![stream.as_str(), seq], |row| row.get(0))
        .optional()
}

/// The last event of `stream`, if any.
pub(crate) fn head(conn: &Connection, stream: Stream) -> rusqlite::Result<Option<ChainLink>> {
    conn.prepare_cached(
        "SELECT seq, hash FROM audit_events WHERE stream = ?1 ORDER BY seq DESC LIMIT 1",
    )?
    .query_row([stream.as_str()], |row| {
        Ok(ChainLink {
            stream,
            seq: row.get(0)?,
            hash: row.get(1)?,
        })
    })
    .optional()
}

fn insert(tx: &Transaction<'_>, row: &EventRow, hash: &str) -> rusqlite::Result<()> {
    let sql = format!(
        "INSERT INTO audit_events ({COLUMNS}, hash) VALUES \
         (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"
    );
    tx.prepare_cached(&sql)?.execute(rusqlite::params![
        row.hash_alg,
        row.stream,
        row.seq,
        row.prev_hash,
        row.occurred_at,
        row.recorded_at,
        row.principal_kind,
        row.principal_id,
        row.session_id,
        row.workstation_id,
        row.action,
        row.outcome,
        row.record_type,
        row.record_id,
        row.old_revision,
        row.new_revision,
        row.reason,
        row.authority,
        row.correlation_id,
        row.details,
        hash,
    ])?;
    Ok(())
}

/// `(principal_kind, principal_id, session_id, workstation_id)`.
fn principal_columns(
    principal: &Principal,
) -> (&'static str, String, Option<String>, Option<String>) {
    let actor = principal.actor();
    let (session_id, workstation_id) = match actor {
        Actor::User {
            session_id,
            workstation_id,
            ..
        } => (
            Some(session_id.to_string()),
            Some(workstation_id.to_string()),
        ),
        Actor::Instrument { .. } | Actor::System { .. } => (None, None),
    };
    (actor.kind(), actor.id_text(), session_id, workstation_id)
}

/// Capability codes, sorted, deduplicated and comma-separated; `None` when empty.
fn authority_text(authority: &[Capability]) -> Option<String> {
    let codes: BTreeSet<&str> = authority
        .iter()
        .map(|capability| capability.as_str())
        .collect();
    if codes.is_empty() {
        return None;
    }
    Some(codes.into_iter().collect::<Vec<_>>().join(","))
}
