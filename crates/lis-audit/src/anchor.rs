//! Anchors: seals written off the host, and their comparison with the local
//! chain (NFR-18, ADR 0008, M4 design brief §4). This module is the one home
//! of the `vibe-tox-anchor-1` format, specified for readers outside the app in
//! `docs/architecture/backup-and-export-formats.md` (Audit anchors).
//!
//! An anchor is one JSON line holding exactly these keys, in this order:
//! `format`, `installation`, `stream`, `seal`, `firstSeq`, `lastSeq`,
//! `prevHash`, `headHash`, `sealedAt` and `hashAlg`. It carries digests and
//! sequence numbers only: no event content, actions, principals or record
//! identifiers, so anchor files hold no PHI. [`parse_line`] is strict: an
//! unknown, missing or repeated key, another format or algorithm, or a
//! malformed value is an error, never ignored.
//!
//! [`compare_anchors`] gives each anchor a raw status against the local chain.
//! Explaining a mismatch by a restore (`lost_interval`,
//! `unverified_restore_boundary`) needs restore records and belongs to
//! `lis-ops`; every per-anchor status carries the anchor's index so that code
//! can read its sequence range, head hash and seal time.

use std::collections::{BTreeMap, HashMap, HashSet};

use lis_core::{InstallationId, UtcTimestamp};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::chain::{GENESIS_HASH, HASH_ALG};
use crate::model::{AuditError, Stream, is_sha256_hex};
use crate::repo::stored_hash;
use crate::seal::{Seal, seals};
use crate::verify::{Finding, verify};

/// The `format` value of every anchor line.
const ANCHOR_FORMAT: &str = "vibe-tox-anchor-1";

/// One parsed anchor line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Anchor {
    /// The installation whose chain was sealed.
    pub installation: InstallationId,
    /// The seal, exactly as anchored.
    pub seal: Seal,
}

/// The JSON line, in key order. Borrowed text rejects escaped strings, which
/// no valid value needs.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Line<'a> {
    format: &'a str,
    installation: &'a str,
    stream: &'a str,
    seal: i64,
    first_seq: i64,
    last_seq: i64,
    prev_hash: &'a str,
    head_hash: &'a str,
    sealed_at: &'a str,
    hash_alg: &'a str,
}

/// The anchor line of `seal`, without a line terminator.
///
/// Fails with [`AuditError::InvalidAnchor`] for a seal that [`parse_line`]
/// would refuse, such as one read from an edited database, so a written line
/// always parses.
pub fn to_line(seal: &Seal, installation: InstallationId) -> Result<String, AuditError> {
    let installation = installation.to_string();
    let sealed_at = seal.sealed_at.to_string();
    let line = Line {
        format: ANCHOR_FORMAT,
        installation: &installation,
        stream: seal.stream.as_str(),
        seal: seal.seal_no,
        first_seq: seal.first_seq,
        last_seq: seal.last_seq,
        prev_hash: &seal.prev_hash,
        head_hash: &seal.head_hash,
        sealed_at: &sealed_at,
        hash_alg: &seal.hash_alg,
    };
    validate(&line)?;
    serde_json::to_string(&line).map_err(|_| AuditError::InvalidAnchor)
}

/// Parses one anchor line strictly (see the module documentation).
pub fn parse_line(line: &str) -> Result<Anchor, AuditError> {
    let parsed: Line<'_> = serde_json::from_str(line).map_err(|_| AuditError::InvalidAnchor)?;
    validate(&parsed)
}

fn validate(line: &Line<'_>) -> Result<Anchor, AuditError> {
    let valid = line.format == ANCHOR_FORMAT
        && line.hash_alg == HASH_ALG
        && line.seal >= 1
        && line.first_seq >= 1
        && line.last_seq >= line.first_seq
        && is_sha256_hex(line.prev_hash)
        && is_sha256_hex(line.head_hash);
    let (Some(stream), Ok(installation), Ok(sealed_at), true) = (
        Stream::parse(line.stream),
        line.installation.parse::<InstallationId>(),
        UtcTimestamp::parse(line.sealed_at),
        valid,
    ) else {
        return Err(AuditError::InvalidAnchor);
    };
    Ok(Anchor {
        installation,
        seal: Seal {
            stream,
            seal_no: line.seal,
            first_seq: line.first_seq,
            last_seq: line.last_seq,
            prev_hash: line.prev_hash.to_owned(),
            head_hash: line.head_hash.to_owned(),
            sealed_at,
            hash_alg: line.hash_alg.to_owned(),
        },
    })
}

/// The raw result of comparing anchors with the local database.
///
/// `anchor` is an index into the slice passed to [`compare_anchors`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorStatus {
    /// The local event at `lastSeq` has the anchored head hash, the event
    /// before `firstSeq` has the anchored `prevHash`, and the chain is intact
    /// through `lastSeq`.
    Matched {
        /// The anchor.
        anchor: usize,
    },
    /// The local event at `lastSeq` does not exist: a truncated stream, or an
    /// older database swapped in (VAL-40).
    LocalEventsMissing {
        /// The anchor.
        anchor: usize,
    },
    /// The local event exists but the local chain does not reproduce the
    /// anchor: events were edited and the chain recomputed, or the chain is
    /// broken at or before `lastSeq`.
    AnchorMismatch {
        /// The anchor.
        anchor: usize,
    },
    /// Two different anchors claim the same seal of a stream.
    AnchorConflict {
        /// The stream.
        stream: Stream,
        /// The seal both claim.
        seal_no: i64,
    },
    /// No anchor exists for these seal numbers, which lie below the stream's
    /// newest anchor.
    AnchorGap {
        /// The stream.
        stream: Stream,
        /// First missing seal number.
        first_seal_no: i64,
        /// Last missing seal number.
        last_seal_no: i64,
    },
    /// Local seals newer than the stream's newest anchor: anchoring lag, not
    /// tampering.
    Unanchored {
        /// The stream.
        stream: Stream,
        /// First local seal without an anchor.
        first_seal_no: i64,
        /// Newest local seal.
        last_seal_no: i64,
    },
}

impl AnchorStatus {
    /// The status code (M4 design brief §4).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Matched { .. } => "matched",
            Self::LocalEventsMissing { .. } => "local_events_missing",
            Self::AnchorMismatch { .. } => "anchor_mismatch",
            Self::AnchorConflict { .. } => "anchor_conflict",
            Self::AnchorGap { .. } => "anchor_gap",
            Self::Unanchored { .. } => "unanchored",
        }
    }
}

/// Compares `anchors`, all of one installation, with the local chains and
/// seals (NFR-18).
///
/// Returns one per-anchor status for each anchor, in input order (identical
/// anchors read from several targets each get one), then for each stream in
/// [`Stream::ALL`] order its conflicts, gaps and unanchored range. The caller
/// keeps anchors of other installations out and reports them itself.
pub fn compare_anchors(
    conn: &Connection,
    anchors: &[Anchor],
) -> Result<Vec<AnchorStatus>, AuditError> {
    let mut first_broken = HashMap::new();
    for stream in Stream::ALL {
        if anchors.iter().any(|anchor| anchor.seal.stream == stream) {
            first_broken.insert(stream, first_broken_seq(conn, stream)?);
        }
    }
    let mut statuses = Vec::with_capacity(anchors.len());
    for (index, anchor) in anchors.iter().enumerate() {
        let broken = first_broken.get(&anchor.seal.stream).copied().flatten();
        statuses.push(anchor_status(conn, index, &anchor.seal, broken)?);
    }
    for stream in Stream::ALL {
        lineage_statuses(conn, stream, anchors, &mut statuses)?;
    }
    Ok(statuses)
}

/// The lowest sequence number at which verification finds the chain broken.
fn first_broken_seq(conn: &Connection, stream: Stream) -> Result<Option<i64>, AuditError> {
    let report = verify(conn, stream)?;
    Ok(report
        .findings
        .iter()
        .filter_map(|finding| match finding {
            Finding::HashMismatch { seq }
            | Finding::BrokenLink { seq }
            | Finding::UnknownAlgorithm { seq }
            | Finding::MissingEvents { first: seq, .. } => Some(*seq),
            Finding::SealMismatch { .. } => None,
        })
        .min())
}

fn anchor_status(
    conn: &Connection,
    anchor: usize,
    seal: &Seal,
    first_broken: Option<i64>,
) -> Result<AnchorStatus, AuditError> {
    let Some(head_hash) = stored_hash(conn, seal.stream, seal.last_seq)? else {
        return Ok(AnchorStatus::LocalEventsMissing { anchor });
    };
    let prev_hash = if seal.first_seq == 1 {
        Some(GENESIS_HASH.to_owned())
    } else {
        stored_hash(conn, seal.stream, seal.first_seq - 1)?
    };
    let matched = head_hash == seal.head_hash
        && prev_hash.as_deref() == Some(seal.prev_hash.as_str())
        && first_broken.is_none_or(|seq| seq > seal.last_seq);
    Ok(if matched {
        AnchorStatus::Matched { anchor }
    } else {
        AnchorStatus::AnchorMismatch { anchor }
    })
}

/// Conflicts, gaps and the unanchored range of one stream.
fn lineage_statuses(
    conn: &Connection,
    stream: Stream,
    anchors: &[Anchor],
    statuses: &mut Vec<AnchorStatus>,
) -> Result<(), AuditError> {
    let mut by_seal: BTreeMap<i64, HashSet<&Anchor>> = BTreeMap::new();
    for anchor in anchors.iter().filter(|anchor| anchor.seal.stream == stream) {
        by_seal
            .entry(anchor.seal.seal_no)
            .or_default()
            .insert(anchor);
    }
    let mut next_expected = 1;
    for (&seal_no, claims) in &by_seal {
        if claims.len() > 1 {
            statuses.push(AnchorStatus::AnchorConflict { stream, seal_no });
        }
        if seal_no > next_expected {
            statuses.push(AnchorStatus::AnchorGap {
                stream,
                first_seal_no: next_expected,
                last_seal_no: seal_no - 1,
            });
        }
        next_expected = seal_no + 1;
    }
    let local = seals(conn, stream, next_expected - 1)?;
    if let (Some(first), Some(last)) = (local.first(), local.last()) {
        statuses.push(AnchorStatus::Unanchored {
            stream,
            first_seal_no: first.seal_no,
            last_seal_no: last.seal_no,
        });
    }
    Ok(())
}
