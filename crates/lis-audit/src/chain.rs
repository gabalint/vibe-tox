//! The hash-chain input format, `sha256-v1`. This is a long-lived contract:
//! never change it in place; add a new `hash_alg` version instead.
//!
//! An event's `hash` is the lowercase hex SHA-256 of its canonical bytes. The
//! canonical bytes are the concatenation of these 20 fields, in this order,
//! each encoded as described below:
//!
//! | # | Field | Kind |
//! | --- | --- | --- |
//! | 1 | `hash_alg` (`sha256-v1`) | text |
//! | 2 | `stream` | text |
//! | 3 | `seq` | integer |
//! | 4 | `prev_hash` | text |
//! | 5 | `occurred_at` | text |
//! | 6 | `recorded_at` | text |
//! | 7 | `principal_kind` | text |
//! | 8 | `principal_id` | text |
//! | 9 | `session_id` | optional text |
//! | 10 | `workstation_id` | optional text |
//! | 11 | `action` | text |
//! | 12 | `outcome` | text |
//! | 13 | `record_type` | optional text |
//! | 14 | `record_id` | optional text |
//! | 15 | `old_revision` | optional integer |
//! | 16 | `new_revision` | optional integer |
//! | 17 | `reason` | optional text |
//! | 18 | `authority` | optional text |
//! | 19 | `correlation_id` | text |
//! | 20 | `details` | text |
//!
//! Encoding:
//!
//! - A present value is the byte `0x01`, then the length of its UTF-8 bytes as
//!   a 4-byte big-endian unsigned integer, then those bytes.
//! - An absent (NULL) value is the single byte `0x00`.
//! - An integer is encoded as the text of its base-10 representation: ASCII
//!   digits, a leading `-` when negative, no leading zeros, no `+`.
//! - Every text value is hashed exactly as stored in `audit_events`.
//!   Timestamps use the canonical `UtcTimestamp` form, and `details` is the
//!   stored JSON text, which never contains floats.
//!
//! The first event of a stream has `seq` 1 and a `prev_hash` of 64 zeros
//! ([`GENESIS_HASH`]). Every later event's `prev_hash` is the stored `hash` of
//! the event at `seq - 1`. Seals and anchors (M4) are computed over the stored
//! hashes, so adding them does not change this format.

use rusqlite::Row;
use sha2::{Digest, Sha256};

/// The current algorithm and serialization version.
pub const HASH_ALG: &str = "sha256-v1";

/// The predecessor hash of the first event in a stream.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Every hashed column of one event, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventRow {
    pub(crate) hash_alg: String,
    pub(crate) stream: String,
    pub(crate) seq: i64,
    pub(crate) prev_hash: String,
    pub(crate) occurred_at: String,
    pub(crate) recorded_at: String,
    pub(crate) principal_kind: String,
    pub(crate) principal_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) workstation_id: Option<String>,
    pub(crate) action: String,
    pub(crate) outcome: String,
    pub(crate) record_type: Option<String>,
    pub(crate) record_id: Option<String>,
    pub(crate) old_revision: Option<i64>,
    pub(crate) new_revision: Option<i64>,
    pub(crate) reason: Option<String>,
    pub(crate) authority: Option<String>,
    pub(crate) correlation_id: String,
    pub(crate) details: String,
}

/// The column list shared by the insert and the verification read, in hash order.
pub(crate) const COLUMNS: &str = "hash_alg, stream, seq, prev_hash, occurred_at, recorded_at, \
     principal_kind, principal_id, session_id, workstation_id, action, outcome, record_type, \
     record_id, old_revision, new_revision, reason, authority, correlation_id, details";

impl EventRow {
    /// Reads the [`COLUMNS`] at indexes 0..20 of `row`.
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            hash_alg: row.get(0)?,
            stream: row.get(1)?,
            seq: row.get(2)?,
            prev_hash: row.get(3)?,
            occurred_at: row.get(4)?,
            recorded_at: row.get(5)?,
            principal_kind: row.get(6)?,
            principal_id: row.get(7)?,
            session_id: row.get(8)?,
            workstation_id: row.get(9)?,
            action: row.get(10)?,
            outcome: row.get(11)?,
            record_type: row.get(12)?,
            record_id: row.get(13)?,
            old_revision: row.get(14)?,
            new_revision: row.get(15)?,
            reason: row.get(16)?,
            authority: row.get(17)?,
            correlation_id: row.get(18)?,
            details: row.get(19)?,
        })
    }

    /// The `sha256-v1` canonical bytes described in the module documentation.
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let seq = self.seq.to_string();
        let old_revision = self.old_revision.map(|revision| revision.to_string());
        let new_revision = self.new_revision.map(|revision| revision.to_string());
        let fields = [
            Some(self.hash_alg.as_str()),
            Some(self.stream.as_str()),
            Some(seq.as_str()),
            Some(self.prev_hash.as_str()),
            Some(self.occurred_at.as_str()),
            Some(self.recorded_at.as_str()),
            Some(self.principal_kind.as_str()),
            Some(self.principal_id.as_str()),
            self.session_id.as_deref(),
            self.workstation_id.as_deref(),
            Some(self.action.as_str()),
            Some(self.outcome.as_str()),
            self.record_type.as_deref(),
            self.record_id.as_deref(),
            old_revision.as_deref(),
            new_revision.as_deref(),
            self.reason.as_deref(),
            self.authority.as_deref(),
            Some(self.correlation_id.as_str()),
            Some(self.details.as_str()),
        ];
        let mut out = Vec::with_capacity(512);
        for field in fields {
            push_field(&mut out, field);
        }
        out
    }

    /// Lowercase hex SHA-256 of the canonical bytes.
    pub(crate) fn compute_hash(&self) -> String {
        hex::encode(Sha256::digest(self.canonical_bytes()))
    }
}

fn push_field(out: &mut Vec<u8>, value: Option<&str>) {
    let Some(text) = value else {
        out.push(0x00);
        return;
    };
    // SQLite caps values at 1 GB (SQLITE_MAX_LENGTH), so every stored length fits in a u32.
    let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
    out.push(0x01);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_row() -> EventRow {
        EventRow {
            hash_alg: "sha256-v1".into(),
            stream: "clinical".into(),
            seq: 1,
            prev_hash: GENESIS_HASH.into(),
            occurred_at: "2026-10-12T10:00:00.000000000Z".into(),
            recorded_at: "2026-10-12T10:00:00.000000000Z".into(),
            principal_kind: "user".into(),
            principal_id: "01900000-0000-7000-8000-000000000001".into(),
            session_id: Some("01900000-0000-7000-8000-000000000002".into()),
            workstation_id: Some("01900000-0000-7000-8000-000000000003".into()),
            action: "specimen.receive".into(),
            outcome: "succeeded".into(),
            record_type: Some("specimen".into()),
            record_id: Some("01900000-0000-7000-8000-000000000004".into()),
            old_revision: None,
            new_revision: Some(1),
            reason: None,
            authority: Some("accession".into()),
            correlation_id: "01900000-0000-7000-8000-000000000005".into(),
            details: "{}".into(),
        }
    }

    /// The canonical bytes of the golden event, written out by hand from the
    /// format table, one line per field.
    fn golden_bytes() -> Vec<u8> {
        [
            b"\x01\x00\x00\x00\x09sha256-v1".as_slice(),
            b"\x01\x00\x00\x00\x08clinical",
            b"\x01\x00\x00\x00\x011",
            b"\x01\x00\x00\x00\x400000000000000000000000000000000000000000000000000000000000000000",
            b"\x01\x00\x00\x00\x1e2026-10-12T10:00:00.000000000Z",
            b"\x01\x00\x00\x00\x1e2026-10-12T10:00:00.000000000Z",
            b"\x01\x00\x00\x00\x04user",
            b"\x01\x00\x00\x00\x2401900000-0000-7000-8000-000000000001",
            b"\x01\x00\x00\x00\x2401900000-0000-7000-8000-000000000002",
            b"\x01\x00\x00\x00\x2401900000-0000-7000-8000-000000000003",
            b"\x01\x00\x00\x00\x10specimen.receive",
            b"\x01\x00\x00\x00\x09succeeded",
            b"\x01\x00\x00\x00\x08specimen",
            b"\x01\x00\x00\x00\x2401900000-0000-7000-8000-000000000004",
            b"\x00",
            b"\x01\x00\x00\x00\x011",
            b"\x00",
            b"\x01\x00\x00\x00\x09accession",
            b"\x01\x00\x00\x00\x2401900000-0000-7000-8000-000000000005",
            b"\x01\x00\x00\x00\x02{}",
        ]
        .concat()
    }

    #[test]
    fn golden_vector_canonical_bytes_are_stable() {
        assert_eq!(golden_row().canonical_bytes(), golden_bytes());
    }

    #[test]
    fn golden_vector_hash_is_stable() {
        // Computed independently from golden_bytes() with `shasum -a 256`.
        assert_eq!(golden_row().compute_hash(), GOLDEN_HASH);
    }

    const GOLDEN_HASH: &str = "ac1f0df2609c488cc06475e88feac1c1ab293969768f2206bd669dfc1cf8d55a";

    /// The format document restates this format for readers outside the app; it must
    /// name the same columns in the same order and the same test vector.
    #[test]
    fn format_document_restates_the_golden_vector() {
        let document = include_str!("../../../docs/architecture/backup-and-export-formats.md");
        let columns: Vec<String> = COLUMNS
            .split(", ")
            .map(|column| format!("`{column}`"))
            .collect();
        assert!(document.contains(&format!(
            "these 20 columns of the same row, in this order: {}.",
            columns.join(", ")
        )));
        let row = golden_row();
        let elided = |id: &Option<String>| format!("`…{}`", &id.as_deref().unwrap()[32..]);
        let restated = [
            format!("a `{}` event with `seq` {}", row.stream, row.seq),
            format!("both times `{}`", row.occurred_at),
            format!("principal `{}` `{}`", row.principal_kind, row.principal_id),
            format!("session {}", elided(&row.session_id)),
            format!("workstation {}", elided(&row.workstation_id)),
            format!("action `{}`, outcome `{}`", row.action, row.outcome),
            format!(
                "record `{}` {}",
                row.record_type.as_deref().unwrap(),
                elided(&row.record_id)
            ),
            format!(
                "no old revision, new revision {}, no reason",
                row.new_revision.unwrap()
            ),
            format!("authority `{}`", row.authority.as_deref().unwrap()),
            format!("correlation {}", elided(&Some(row.correlation_id.clone()))),
            format!("details `{}`", row.details),
            format!("hashes to `{GOLDEN_HASH}`"),
        ];
        assert_eq!(row.occurred_at, row.recorded_at);
        assert_eq!(row.prev_hash, GENESIS_HASH);
        for text in restated {
            assert!(
                document.contains(&text),
                "the document does not say: {text}"
            );
        }
    }

    #[test]
    fn absent_and_empty_values_hash_differently() {
        let absent = golden_row();
        let empty = EventRow {
            reason: Some(String::new()),
            ..golden_row()
        };
        assert_ne!(absent.compute_hash(), empty.compute_hash());
    }

    #[test]
    fn field_boundaries_cannot_be_shifted() {
        let one = EventRow {
            record_type: Some("ab".into()),
            record_id: Some("c".into()),
            ..golden_row()
        };
        let other = EventRow {
            record_type: Some("a".into()),
            record_id: Some("bc".into()),
            ..golden_row()
        };
        assert_ne!(one.compute_hash(), other.compute_hash());
    }
}
