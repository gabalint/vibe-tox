//! Transactional outbox: durable follow-up work (NFR-02, NFR-06, REQ-INT-008).
//!
//! A command enqueues an item in the same transaction as its change. Workers
//! (M1, in the engine) [`lease`] items with an expiry, then [`complete`] or
//! [`fail`] them. Each lease counts as an attempt, so an item whose worker
//! keeps crashing still reaches the dead-letter state. Expired leases become
//! leasable again. Outbox operations are technical state, not clinical state,
//! so they are not audited; workers log their outcomes.
//!
//! The lease, complete and fail operations run as writes; [`dead_letters`] and
//! [`backlog`] run on read connections.

use std::time::Duration;

use lis_core::{Id, UtcTimestamp};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, Type, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::Value;

use crate::context::CommandContext;
use crate::error::StoreError;

/// Attempts allowed before an item is dead-lettered.
pub const MAX_ATTEMPTS: u32 = 8;

/// Identifier of an outbox item.
pub type OutboxItemId = Id<OutboxItem>;

/// Work to do after the current transaction commits.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxItem {
    /// The worker queue, such as `report.deliver`.
    pub topic: &'static str,
    /// Identity of the logical effect within the topic; repeats are deduplicated.
    pub dedup_key: String,
    /// What the worker needs, as references and versions. Kept as JSON text.
    pub payload: Value,
}

/// Where an item is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxState {
    /// Waiting until `available_at`.
    Pending,
    /// Held by a worker until the lease expires.
    Leased,
    /// Finished.
    Done,
    /// Failed permanently or ran out of attempts; visible to the lab.
    Dead,
}

impl FromSql for OutboxState {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "pending" => Ok(Self::Pending),
            "leased" => Ok(Self::Leased),
            "done" => Ok(Self::Done),
            "dead" => Ok(Self::Dead),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// An item leased to a worker. `attempt` fences `complete` and `fail`.
#[derive(Debug, Clone, PartialEq)]
pub struct LeasedItem {
    /// The item.
    pub id: OutboxItemId,
    /// Its topic.
    pub topic: String,
    /// Its dedup key.
    pub dedup_key: String,
    /// Its payload.
    pub payload: Value,
    /// Which attempt this lease is, from 1.
    pub attempt: u32,
}

/// How an attempt failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Try again at `retry_at`, unless attempts are exhausted.
    Retry {
        /// Code-defined, PHI-free error code, such as `timeout`.
        error_code: &'static str,
        /// When the item becomes leasable again (the worker's backoff).
        retry_at: UtcTimestamp,
    },
    /// Never retry; dead-letter now.
    Permanent {
        /// Code-defined, PHI-free error code.
        error_code: &'static str,
    },
}

/// A dead-lettered item the lab must act on. The payload is left out on purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadLetter {
    /// The item.
    pub id: OutboxItemId,
    /// Its topic.
    pub topic: String,
    /// Its dedup key.
    pub dedup_key: String,
    /// Attempts made.
    pub attempts: u32,
    /// The last error code, such as `lease_expired`.
    pub last_error_code: Option<String>,
    /// When it was dead-lettered.
    pub updated_at: UtcTimestamp,
}

/// Unfinished work on one topic, for health checks (NFR-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicBacklog {
    /// The topic.
    pub topic: String,
    /// Pending and leased items.
    pub outstanding: u32,
    /// Creation time of the oldest outstanding item.
    pub oldest_created_at: UtcTimestamp,
    /// How long the oldest item has waited (zero if the clock is behind it).
    pub oldest_age: Duration,
}

pub(crate) fn enqueue(
    tx: &Transaction<'_>,
    item: &OutboxItem,
    now: UtcTimestamp,
) -> Result<OutboxItemId, StoreError> {
    let existing = tx
        .prepare_cached("SELECT id, payload FROM store_outbox WHERE topic = ?1 AND dedup_key = ?2")?
        .query_row(params![item.topic, item.dedup_key], |row| {
            Ok((row.get::<_, OutboxItemId>(0)?, payload_at(row, 1)?))
        })
        .optional()?;
    if let Some((id, stored)) = existing {
        // Compared as JSON values, so key order never makes equal payloads differ.
        return if stored == item.payload {
            Ok(id)
        } else {
            Err(StoreError::OutboxConflict)
        };
    }
    let payload = item.payload.to_string();
    let id = OutboxItemId::generate();
    tx.prepare_cached(
        "INSERT INTO store_outbox (id, topic, dedup_key, payload, state, attempts, max_attempts, \
         available_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5, ?6, ?6, ?6)",
    )?
    .execute(params![id, item.topic, item.dedup_key, payload, MAX_ATTEMPTS, now])?;
    Ok(id)
}

/// Leases up to `limit` ready items of `topic` for `lease_for`, oldest first.
///
/// Ready means pending and due, or leased with an expired lease. An expired
/// lease that used the last attempt is dead-lettered as `lease_expired`.
pub fn lease(
    ctx: &CommandContext<'_>,
    topic: &str,
    limit: u32,
    lease_for: Duration,
) -> Result<Vec<LeasedItem>, StoreError> {
    let now = ctx.now();
    let expires_at = now
        .checked_add(lease_for)
        .ok_or(StoreError::TimeOutOfRange)?;
    let tx = ctx.tx();
    tx.prepare_cached(
        "UPDATE store_outbox SET state = 'dead', lease_expires_at = NULL, \
         last_error_code = 'lease_expired', updated_at = ?2 \
         WHERE topic = ?1 AND state = 'leased' AND lease_expires_at <= ?2 \
         AND attempts >= max_attempts",
    )?
    .execute(params![topic, now])?;
    let ready = tx
        .prepare_cached(
            "SELECT id, topic, dedup_key, payload, attempts + 1 FROM store_outbox \
             WHERE topic = ?1 AND ((state = 'pending' AND available_at <= ?2) \
             OR (state = 'leased' AND lease_expires_at <= ?2)) \
             ORDER BY available_at, id LIMIT ?3",
        )?
        .query_map(params![topic, now, limit], leased_item)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut mark = tx.prepare_cached(
        "UPDATE store_outbox SET state = 'leased', attempts = ?2, lease_expires_at = ?3, \
         updated_at = ?4 WHERE id = ?1",
    )?;
    for item in &ready {
        mark.execute(params![item.id, item.attempt, expires_at, now])?;
    }
    Ok(ready)
}

fn leased_item(row: &Row<'_>) -> rusqlite::Result<LeasedItem> {
    Ok(LeasedItem {
        id: row.get(0)?,
        topic: row.get(1)?,
        dedup_key: row.get(2)?,
        payload: payload_at(row, 3)?,
        attempt: row.get(4)?,
    })
}

/// Parses the JSON payload text in column `index`.
fn payload_at(row: &Row<'_>, index: usize) -> rusqlite::Result<Value> {
    let text: String = row.get(index)?;
    serde_json::from_str(&text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

/// Marks a leased item done.
pub fn complete(ctx: &CommandContext<'_>, item: &LeasedItem) -> Result<(), StoreError> {
    let changed = ctx
        .tx()
        .prepare_cached(
            "UPDATE store_outbox SET state = 'done', lease_expires_at = NULL, updated_at = ?3 \
             WHERE id = ?1 AND state = 'leased' AND attempts = ?2",
        )?
        .execute(params![item.id, item.attempt, ctx.now()])?;
    if changed == 0 {
        return Err(StoreError::OutboxNotLeased);
    }
    Ok(())
}

/// Records a failed attempt and returns the item's new state: `Pending` for a
/// retry, `Dead` for a permanent failure or exhausted attempts.
pub fn fail(
    ctx: &CommandContext<'_>,
    item: &LeasedItem,
    failure: Failure,
) -> Result<OutboxState, StoreError> {
    let (error_code, retry_at) = match failure {
        Failure::Retry {
            error_code,
            retry_at,
        } => (error_code, Some(retry_at)),
        Failure::Permanent { error_code } => (error_code, None),
    };
    ctx.tx()
        .prepare_cached(
            "UPDATE store_outbox SET \
             state = CASE WHEN ?3 IS NULL OR attempts >= max_attempts THEN 'dead' ELSE 'pending' END, \
             available_at = coalesce(?3, available_at), lease_expires_at = NULL, \
             last_error_code = ?4, updated_at = ?5 \
             WHERE id = ?1 AND state = 'leased' AND attempts = ?2 RETURNING state",
        )?
        .query_row(
            params![item.id, item.attempt, retry_at, error_code, ctx.now()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::OutboxNotLeased)
}

/// The oldest `limit` dead-lettered items, so a backlog is never loaded whole.
pub fn dead_letters(conn: &Connection, limit: u32) -> Result<Vec<DeadLetter>, StoreError> {
    let letters = conn
        .prepare_cached(
            "SELECT id, topic, dedup_key, attempts, last_error_code, updated_at FROM store_outbox \
             WHERE state = 'dead' ORDER BY updated_at, id LIMIT ?1",
        )?
        .query_map([limit], |row| {
            Ok(DeadLetter {
                id: row.get(0)?,
                topic: row.get(1)?,
                dedup_key: row.get(2)?,
                attempts: row.get(3)?,
                last_error_code: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(letters)
}

/// Outstanding (pending or leased) items per topic, with the oldest item's age at `now`.
pub fn backlog(conn: &Connection, now: UtcTimestamp) -> Result<Vec<TopicBacklog>, StoreError> {
    let backlog = conn
        .prepare_cached(
            "SELECT topic, count(*), min(created_at) FROM store_outbox \
             WHERE state IN ('pending', 'leased') GROUP BY topic ORDER BY topic",
        )?
        .query_map([], |row| {
            let oldest_created_at: UtcTimestamp = row.get(2)?;
            Ok(TopicBacklog {
                topic: row.get(0)?,
                outstanding: row.get(1)?,
                oldest_created_at,
                oldest_age: now.saturating_duration_since(oldest_created_at),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(backlog)
}
