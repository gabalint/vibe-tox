//! Seals of the audit chains (NFR-18, ADR 0008).
//!
//! A seal records a stream's head: the range of sequence numbers it adds to
//! the previous seal, the previous seal's head hash and the stored hash of the
//! event at its last sequence number. Because every event hash commits to its
//! predecessor, the head hash commits to the whole prefix of the stream, so a
//! seal needs no separate segment digest. Seals of a stream form their own
//! chain, enforced by the insert trigger of migration 0002; anchors are seals
//! written off the host ([`crate::anchor`]).
//!
//! Sealing is not audited: an audit event about a seal would itself always be
//! unsealed (M4 design brief, D6).

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::chain::{GENESIS_HASH, HASH_ALG};
use crate::model::{AuditError, Stream};
use crate::repo::head;

/// One sealed range of a stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Seal {
    /// The sealed stream.
    pub stream: Stream,
    /// Position in the stream's chain of seals, from 1 without gaps.
    pub seal_no: i64,
    /// First sequence number this seal adds: the previous seal's `last_seq` + 1, or 1.
    pub first_seq: i64,
    /// The stream's head when sealed.
    pub last_seq: i64,
    /// The previous seal's `head_hash`, or the genesis hash for seal 1.
    pub prev_hash: String,
    /// The stored hash of the event at `last_seq`.
    pub head_hash: String,
    /// When the seal was taken (host clock).
    pub sealed_at: UtcTimestamp,
    /// The chain's hash algorithm, `sha256-v1`.
    pub hash_alg: String,
}

const SEAL_COLUMNS: &str =
    "seal_no, first_seq, last_seq, prev_hash, head_hash, sealed_at, hash_alg";

impl Seal {
    /// Reads [`SEAL_COLUMNS`] of a row of `stream`.
    fn from_row(stream: Stream, row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            stream,
            seal_no: row.get(0)?,
            first_seq: row.get(1)?,
            last_seq: row.get(2)?,
            prev_hash: row.get(3)?,
            head_hash: row.get(4)?,
            sealed_at: row.get(5)?,
            hash_alg: row.get(6)?,
        })
    }
}

/// Where the seal after `previous` starts: `(seal_no, first_seq, prev_hash)`.
/// The insert trigger of migration 0002 enforces the same rule.
pub(crate) fn next_position(previous: Option<&Seal>) -> (i64, i64, &str) {
    match previous {
        Some(previous) => (
            previous.seal_no + 1,
            previous.last_seq + 1,
            previous.head_hash.as_str(),
        ),
        None => (1, 1, GENESIS_HASH),
    }
}

/// Seals every stream whose head is beyond its last seal, inside the
/// caller's transaction, and returns the new seals in [`Stream::ALL`] order.
///
/// A stream without new events gets no seal, so calling this again at once
/// returns nothing.
pub fn seal(tx: &Transaction<'_>, now: UtcTimestamp) -> Result<Vec<Seal>, AuditError> {
    let mut sealed = Vec::new();
    for stream in Stream::ALL {
        let Some(head) = head(tx, stream)? else {
            continue;
        };
        let previous = last_seal(tx, stream)?;
        if previous
            .as_ref()
            .is_some_and(|seal| seal.last_seq >= head.seq)
        {
            continue;
        }
        let (seal_no, first_seq, prev_hash) = next_position(previous.as_ref());
        let seal = Seal {
            stream,
            seal_no,
            first_seq,
            last_seq: head.seq,
            prev_hash: prev_hash.to_owned(),
            head_hash: head.hash,
            sealed_at: now,
            hash_alg: HASH_ALG.to_owned(),
        };
        insert(tx, &seal)?;
        sealed.push(seal);
    }
    Ok(sealed)
}

/// The seals of `stream` numbered above `after_seal_no`, in order.
/// Pass 0 for every seal.
pub fn seals(
    conn: &Connection,
    stream: Stream,
    after_seal_no: i64,
) -> Result<Vec<Seal>, AuditError> {
    let sql = format!(
        "SELECT {SEAL_COLUMNS} FROM audit_seals WHERE stream = ?1 AND seal_no > ?2 \
         ORDER BY seal_no"
    );
    let seals = conn
        .prepare_cached(&sql)?
        .query_map(params![stream.as_str(), after_seal_no], |row| {
            Seal::from_row(stream, row)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(seals)
}

/// The newest seal of `stream`, if any: what a backup manifest records
/// without reading every seal.
pub fn last_seal(conn: &Connection, stream: Stream) -> Result<Option<Seal>, AuditError> {
    let sql = format!(
        "SELECT {SEAL_COLUMNS} FROM audit_seals WHERE stream = ?1 \
         ORDER BY seal_no DESC LIMIT 1"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([stream.as_str()], |row| Seal::from_row(stream, row))
        .optional()?)
}

fn insert(tx: &Transaction<'_>, seal: &Seal) -> rusqlite::Result<()> {
    tx.prepare_cached(
        "INSERT INTO audit_seals (stream, seal_no, first_seq, last_seq, prev_hash, head_hash, \
         sealed_at, hash_alg) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        seal.stream.as_str(),
        seal.seal_no,
        seal.first_seq,
        seal.last_seq,
        seal.prev_hash,
        seal.head_hash,
        seal.sealed_at,
        seal.hash_alg,
    ])?;
    Ok(())
}
