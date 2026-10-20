//! Audit trail: append hash-chained audit events inside the committing
//! transaction and verify the chains (REQ-OPS-001, NFR-18, ADR 0008).
//!
//! - [`append`] adds an event to its [`Stream`] in the caller's `BEGIN
//!   IMMEDIATE` transaction, with the next per-stream sequence number, the
//!   predecessor's hash and a SHA-256 over the versioned canonical
//!   serialization documented in [`chain`] (`sha256-v1`).
//! - [`list`] pages through a stream, newest first, for the audit viewer.
//! - [`verify`] recomputes a stream's chain and reports edited, deleted,
//!   inserted and reordered events, and seals that disagree with the chain,
//!   as [`Finding`]s.
//! - [`seal`] records each stream's head in `audit_seals` (NFR-18, ADR 0008);
//!   [`heads`], [`seals`], [`last_seal`] and [`event_recorded_at`] serve backups, exports
//!   and anchor lag (REQ-OPS-007, REQ-OPS-010).
//! - [`to_line`] and [`parse_line`] are the one home of the digest-only
//!   `vibe-tox-anchor-1` line; [`compare_anchors`] gives each anchor a raw
//!   status against the local chain.
//!
//! Invariants: `audit_events` and `audit_seals` are append-only (triggers
//! abort `UPDATE` and `DELETE`); each stream's sequence starts at 1 and has no
//! gaps; each seal continues the previous one and names the stored hash of the
//! event at its last sequence number (insert trigger); events hold record
//! identifiers and non-PHI references, never clinical content; anchors hold
//! digests and sequence numbers only. The triggers stop application bugs, not
//! the OS administrator: verification detects edits, and off-host anchors
//! detect recomputation of the chain and truncation. Seals and anchors are
//! computed over stored hashes and need no change to the hash format.
//!
//! Most commands do not call [`append`] directly: `lis-store`'s
//! `CommandContext::audit` fills in the [`Attribution`].
//! [`AuditDetail`] is the only details value API; it admits typed IDs, codes,
//! integers, booleans, validated digests and machine identifiers, plus an
//! explicitly classified workforce username, never arbitrary strings or JSON
//! values.
//!
//! See `docs/governance/security-audit-logging.md` (Audit trail).

mod anchor;
pub mod chain;
mod labels;
mod list;
mod model;
mod record_events;
mod repo;
mod seal;
mod verify;

use lis_core::{Migration, ModuleSchema};

pub use anchor::{Anchor, AnchorStatus, compare_anchors, parse_line, to_line};
pub use labels::{action_label, record_type_label};
pub use list::{EventFilter, StoredEvent, actions, list, record_types};
pub use model::{
    Attribution, AuditDetail, AuditError, AuditEvent, ChainLink, MAX_DETAILS_BYTES,
    MAX_MACHINE_TOKEN_BYTES, Outcome, Stream,
};
pub use record_events::{RecordEvent, succeeded_record_events};
pub use repo::{append, event_recorded_at, heads};
pub use seal::{Seal, last_seal, seal, seals};
pub use verify::{Finding, VerifyReport, verify};

/// This module's migrations, in version order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "audit_events",
        sql: include_str!("../migrations/0001_audit_events.sql"),
    },
    Migration {
        version: 2,
        name: "audit_seals",
        sql: include_str!("../migrations/0002_audit_seals.sql"),
    },
    Migration {
        version: 3,
        name: "record_event_lookup",
        sql: include_str!("../migrations/0003_record_event_lookup.sql"),
    },
];

/// This module's schema descriptor.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "audit",
    table_prefix: "audit_",
    migrations: MIGRATIONS,
};

#[cfg(test)]
mod tests;
