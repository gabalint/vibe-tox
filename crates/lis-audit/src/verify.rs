//! Chain verification (NFR-18, ADR 0008).
//!
//! Verification recomputes every hash of a stream in sequence order and checks
//! each link. How tampering shows up:
//!
//! | Tampering | Findings |
//! | --- | --- |
//! | A field of an event edited | `HashMismatch` at that event |
//! | Events deleted | `MissingEvents` for the gap, and `BrokenLink` at the next event |
//! | An event inserted by renumbering later events | `BrokenLink` after the forged event, `HashMismatch` at every renumbered event (and at the forged one unless its hash was recomputed) |
//! | Events reordered | `HashMismatch` and `BrokenLink` at each moved position |
//! | A forged event with a bad hash appended | `HashMismatch` at that event |
//! | The chain recomputed, or its tail deleted, behind a seal | `SealMismatch` at that seal |
//! | A seal edited, deleted or inserted | `SealMismatch` at the first seal that no longer continues the previous one |
//!
//! Every seal must continue the previous one and name the stored hash of the
//! event at its `last_seq`.
//!
//! **Limits on the host.** Someone who recomputes every hash from the point of
//! tampering onwards and rewrites the seals, or who deletes unsealed events
//! from the end of a stream, leaves a chain that verifies. Only off-host
//! anchors ([`crate::compare_anchors`]) detect that, up to the anchor lag. The
//! `head` in the report is the newest event read.

use rusqlite::Connection;

use crate::chain::{COLUMNS, EventRow, GENESIS_HASH, HASH_ALG};
use crate::model::{AuditError, ChainLink, Stream};
use crate::repo::stored_hash;
use crate::seal::{Seal, next_position, seals};

/// One integrity problem found in a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The stored hash does not match the event's fields and position: the
    /// event was edited, moved or forged after it was written.
    HashMismatch {
        /// Sequence number of the event.
        seq: i64,
    },
    /// The event's `prev_hash` is not the stored hash of the event before it:
    /// an earlier event was deleted, inserted, replaced or moved.
    BrokenLink {
        /// Sequence number of the event.
        seq: i64,
    },
    /// No events exist for sequence numbers `first..=last`: they were deleted.
    MissingEvents {
        /// First missing sequence number.
        first: i64,
        /// Last missing sequence number.
        last: i64,
    },
    /// The event names a hash algorithm this build cannot check.
    UnknownAlgorithm {
        /// Sequence number of the event.
        seq: i64,
    },
    /// The seal does not continue the previous seal, or its head hash is not
    /// the stored hash of the event at its last sequence number.
    SealMismatch {
        /// Number of the seal.
        seal_no: i64,
    },
}

/// The result of verifying one stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// The verified stream.
    pub stream: Stream,
    /// Number of events read.
    pub events_checked: u64,
    /// The last event read, which anchors compare against; `None` for an empty stream.
    pub head: Option<ChainLink>,
    /// Problems found: event findings in sequence order, then seal findings in seal order.
    pub findings: Vec<Finding>,
}

impl VerifyReport {
    /// True when no problem was found.
    #[must_use]
    pub fn is_intact(&self) -> bool {
        self.findings.is_empty()
    }

    fn check(&mut self, event: &EventRow, stored_hash: &str) {
        let (expected_seq, expected_prev) = match &self.head {
            Some(previous) => (previous.seq + 1, previous.hash.as_str()),
            None => (1, GENESIS_HASH),
        };
        if event.seq > expected_seq {
            self.findings.push(Finding::MissingEvents {
                first: expected_seq,
                last: event.seq - 1,
            });
        }
        if event.prev_hash != expected_prev {
            self.findings.push(Finding::BrokenLink { seq: event.seq });
        }
        if event.hash_alg != HASH_ALG {
            self.findings
                .push(Finding::UnknownAlgorithm { seq: event.seq });
        } else if event.compute_hash() != stored_hash {
            self.findings.push(Finding::HashMismatch { seq: event.seq });
        }
        self.events_checked += 1;
        self.head = Some(ChainLink {
            stream: self.stream,
            seq: event.seq,
            hash: stored_hash.to_owned(),
        });
    }
}

/// Recomputes and checks the whole chain of `stream`.
///
/// An `Err` means the stream could not be read, which counts as a failed
/// verification, not a pass.
pub fn verify(conn: &Connection, stream: Stream) -> Result<VerifyReport, AuditError> {
    let sql = format!("SELECT {COLUMNS}, hash FROM audit_events WHERE stream = ?1 ORDER BY seq");
    let mut statement = conn.prepare(&sql)?;
    let mut rows = statement.query([stream.as_str()])?;
    let mut report = VerifyReport {
        stream,
        events_checked: 0,
        head: None,
        findings: Vec::new(),
    };
    while let Some(row) = rows.next()? {
        let event = EventRow::from_row(row)?;
        let stored_hash: String = row.get(20)?;
        report.check(&event, &stored_hash);
    }
    check_seals(conn, &mut report)?;
    Ok(report)
}

/// Adds a [`Finding::SealMismatch`] for every seal that disagrees with the
/// seal before it or with the chain.
fn check_seals(conn: &Connection, report: &mut VerifyReport) -> Result<(), AuditError> {
    let mut previous: Option<Seal> = None;
    for seal in seals(conn, report.stream, 0)? {
        let expected = next_position(previous.as_ref());
        let continues = (seal.seal_no, seal.first_seq, seal.prev_hash.as_str()) == expected
            && seal.last_seq >= seal.first_seq;
        let head = stored_hash(conn, report.stream, seal.last_seq)?;
        if !continues || head.as_deref() != Some(seal.head_hash.as_str()) {
            report.findings.push(Finding::SealMismatch {
                seal_no: seal.seal_no,
            });
        }
        previous = Some(seal);
    }
    Ok(())
}
