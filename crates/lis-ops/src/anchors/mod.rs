//! Audit anchors: export of seals to every target, anchor lag, and verification that
//! explains restores (NFR-18, ADR 0008, REQ-OPS-008; M4 design brief §4, §6.1).
//!
//! - **Export.** [`export_pending`] seals the audit streams, then writes the seals each
//!   available target has not received yet, in seal order, as one new JSON Lines file
//!   `VibeToxLIS/anchors/<YYYY-MM-DD>/<YYYYMMDDTHHMMSSZ>-<exportId>.jsonl` (temporary name,
//!   sync, rename, folder sync), records it in `ops_anchor_exports` and its seal ranges,
//!   and writes one `ops.export_anchors` event per pass. Seals wait in the database while
//!   a target is absent and drain in order when it returns.
//! - **Lag.** [`get_anchor_status`] measures, per stream, the oldest event no seal on an
//!   active off-host target covers (§4), and feeds the `anchor_lag` health check.
//! - **Verification.** [`read_anchors`] reads every anchor file under chosen folders,
//!   strictly; [`verify_anchors`] compares them with the local chains
//!   ([`lis_audit::compare_anchors`]) and [`classify`] explains restores from the restore
//!   records it is given (`lost_interval`, `unverified_restore_boundary`).
//!
//! Invariants:
//! - an anchor file is written once under a new name and never modified; anchor lines come
//!   only from [`lis_audit::to_line`], the one home of the format, so they hold the ten
//!   digest-only keys and nothing else;
//! - a seal that cannot be written as a line fails its target's export, never skipped, so a
//!   target's anchors have no holes; the store refuses a range that does not continue the
//!   target's previous one;
//! - no file I/O runs inside a transaction: seal and plan, write files, then record;
//! - only removable and network-share targets count as off-host (D6);
//! - a restore explains a finding only through its records; a restore whose own
//!   `ops.restore_applied` event is not covered by a matched anchor explains nothing for
//!   certain and gives a warning (E8);
//! - audit details hold counts only; logs hold target IDs and codes, never paths.

mod classify;
mod error;
mod export;
mod lag;
mod repo;
mod verify;

use lis_core::Id;

pub use classify::{AnchorResult, Classified, RestoreRecord, classify};
pub use error::AnchorError;
pub use export::{ExportReport, export_pending};
pub use lag::{
    AnchorExportView, AnchorStatusView, LastExportView, SealRangeView, StreamAnchorView,
    get_anchor_status, list_anchor_exports,
};
pub use verify::{
    AnchorFinding, AnchorSummary, AnchorVerification, ChainView, ReadAnchors, read_anchors,
    verify_anchors,
};

/// Audit action of an anchor export pass, scheduled or manual (M4 design brief §2.4).
pub const EXPORT_ANCHORS: &str = "ops.export_anchors";

/// Marker type of anchor export IDs.
pub enum AnchorExport {}
/// Identifies one anchor file written to one target; its file name ends with this ID.
pub type AnchorExportId = Id<AnchorExport>;

#[cfg(test)]
mod tests;
