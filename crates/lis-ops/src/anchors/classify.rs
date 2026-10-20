//! Explaining raw anchor statuses with restore records (M4 design brief §4 verification
//! table, E8). [`lis_audit::compare_anchors`] knows only the chains; after a restore of an
//! older backup set, anchors written between the set and the restore no longer match,
//! and the re-sealed numbers conflict with them. The restore records explain those, and
//! only those.
//!
//! For an anchor (stream `s`, last sequence `L`, sealed at `T`) that is missing locally or
//! does not match:
//! - a restore R with restored head of `s` below `L`, applied after `T`, whose own
//!   `ops.restore_applied` event is covered by a matched anchor, gives `lost_interval`;
//! - the same with R's event not covered gives `unverified_restore_boundary` (a warning);
//! - otherwise the raw status stands.
//!
//! Two different anchors for one seal number are explained when every claimant but one was
//! sealed before a restore and is matched or itself explained; the conflict is then
//! dropped, or reported as `unverified_restore_boundary` while that restore is not anchored.
//!
//! The records must come from `restore::restore_records`, which keeps only records bound
//! to their own `ops.restore_applied` event by its hash and a digest over `applied_at`,
//! the set and the restored heads; an edited record therefore explains nothing.

use std::collections::HashSet;

use lis_audit::{Anchor, AnchorStatus, ChainLink, Stream};
use lis_core::UtcTimestamp;

lis_core::text_enum! {
    /// What a verification concludes about an anchor or a stream's lineage (§4).
    AnchorResult {
        /// The local chain reproduces the anchor.
        Matched = "matched",
        /// Lost with a restore of an older set; expected and counted.
        LostInterval = "lost_interval",
        /// Would be a lost interval, but the restore itself is not anchored yet: a warning.
        UnverifiedRestoreBoundary = "unverified_restore_boundary",
        /// The anchored event does not exist locally: truncation, or an older database
        /// swapped in (VAL-40).
        LocalEventsMissing = "local_events_missing",
        /// The local chain does not reproduce the anchor: edited events.
        AnchorMismatch = "anchor_mismatch",
        /// Seal numbers below the newest anchor that no anchor covers.
        AnchorGap = "anchor_gap",
        /// Two different anchors for one seal that no restore explains.
        AnchorConflict = "anchor_conflict",
        /// Local seals newer than the newest anchor: lag, not tampering.
        Unanchored = "unanchored",
    }
}

impl AnchorResult {
    /// True when the result needs investigation: everything except `matched`, the
    /// expected `lost_interval` and the lag `unanchored` (the `audit_chain` check, §6.1).
    #[must_use]
    pub fn is_problem(self) -> bool {
        !matches!(self, Self::Matched | Self::LostInterval | Self::Unanchored)
    }
}

/// One restore, as plain data from the restore records (`ops_restores`,
/// `ops_restore_heads`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRecord {
    /// When the restored database was put in place.
    pub applied_at: UtcTimestamp,
    /// The head of each stream in the restored set; a stream not listed was empty.
    pub restored_heads: Vec<ChainLink>,
    /// The restore's own `ops.restore_applied` event in the restored database.
    pub applied_event: ChainLink,
}

/// One classified anchor, conflict, gap or unanchored range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classified {
    /// The conclusion.
    pub result: AnchorResult,
    /// The stream.
    pub stream: Stream,
    /// The anchor's seal number, or the first of a range.
    pub seal_no: i64,
    /// The same seal number, or the last of a range.
    pub last_seal_no: i64,
}

/// Classifies the raw `statuses` that [`lis_audit::compare_anchors`] returned for
/// `anchors`, in the same order, using `restores`. Conflicts explained by a verified
/// restore are dropped; every other status gives one entry.
#[must_use]
pub fn classify(
    anchors: &[Anchor],
    statuses: &[AnchorStatus],
    restores: &[RestoreRecord],
) -> Vec<Classified> {
    let explainer = Explainer {
        restores,
        covered: restores
            .iter()
            .map(|restore| is_covered(restore, anchors, statuses))
            .collect(),
    };
    let mut per_anchor = vec![None; anchors.len()];
    for (index, result) in statuses
        .iter()
        .filter_map(|status| explainer.anchor_result(anchors, status))
    {
        if let Some(slot) = per_anchor.get_mut(index) {
            *slot = Some(result);
        }
    }
    statuses
        .iter()
        .filter_map(|status| match *status {
            AnchorStatus::Matched { anchor }
            | AnchorStatus::LocalEventsMissing { anchor }
            | AnchorStatus::AnchorMismatch { anchor } => {
                let seal = &anchors.get(anchor)?.seal;
                let result = per_anchor.get(anchor).copied().flatten()?;
                Some(one(result, seal.stream, seal.seal_no))
            }
            AnchorStatus::AnchorConflict { stream, seal_no } => explainer
                .conflict(&claimants(anchors, &per_anchor, stream, seal_no))
                .map(|result| one(result, stream, seal_no)),
            AnchorStatus::AnchorGap {
                stream,
                first_seal_no,
                last_seal_no,
            } => Some(range(
                AnchorResult::AnchorGap,
                stream,
                first_seal_no,
                last_seal_no,
            )),
            AnchorStatus::Unanchored {
                stream,
                first_seal_no,
                last_seal_no,
            } => Some(range(
                AnchorResult::Unanchored,
                stream,
                first_seal_no,
                last_seal_no,
            )),
        })
        .collect()
}

fn one(result: AnchorResult, stream: Stream, seal_no: i64) -> Classified {
    range(result, stream, seal_no, seal_no)
}

fn range(result: AnchorResult, stream: Stream, seal_no: i64, last_seal_no: i64) -> Classified {
    Classified {
        result,
        stream,
        seal_no,
        last_seal_no,
    }
}

/// True when a matched anchor covers the restore's own event.
fn is_covered(restore: &RestoreRecord, anchors: &[Anchor], statuses: &[AnchorStatus]) -> bool {
    let event = &restore.applied_event;
    statuses.iter().any(|status| match *status {
        AnchorStatus::Matched { anchor } => anchors.get(anchor).is_some_and(|anchor| {
            anchor.seal.stream == event.stream && anchor.seal.last_seq >= event.seq
        }),
        _ => false,
    })
}

/// The distinct anchors claiming `(stream, seal_no)`, with their per-anchor results.
fn claimants<'a>(
    anchors: &'a [Anchor],
    per_anchor: &[Option<AnchorResult>],
    stream: Stream,
    seal_no: i64,
) -> Vec<(&'a Anchor, Option<AnchorResult>)> {
    let mut seen = HashSet::new();
    anchors
        .iter()
        .zip(per_anchor)
        .filter(|(anchor, _)| anchor.seal.stream == stream && anchor.seal.seal_no == seal_no)
        .filter(|(anchor, _)| seen.insert(*anchor))
        .map(|(anchor, result)| (anchor, *result))
        .collect()
}

struct Explainer<'a> {
    restores: &'a [RestoreRecord],
    /// Per restore: its own event is covered by a matched anchor.
    covered: Vec<bool>,
}

impl Explainer<'_> {
    /// The per-anchor result of a per-anchor status, with its anchor index.
    fn anchor_result(
        &self,
        anchors: &[Anchor],
        status: &AnchorStatus,
    ) -> Option<(usize, AnchorResult)> {
        let (anchor, raw) = match *status {
            AnchorStatus::Matched { anchor } => return Some((anchor, AnchorResult::Matched)),
            AnchorStatus::LocalEventsMissing { anchor } => {
                (anchor, AnchorResult::LocalEventsMissing)
            }
            AnchorStatus::AnchorMismatch { anchor } => (anchor, AnchorResult::AnchorMismatch),
            _ => return None,
        };
        let seal = &anchors.get(anchor)?.seal;
        let lost = self.explain(|restore| {
            restored_head(restore, seal.stream) < seal.last_seq
                && seal.sealed_at < restore.applied_at
        });
        Some((anchor, lost.unwrap_or(raw)))
    }

    /// `None` when the conflict is explained by verified restores.
    fn conflict(&self, claims: &[(&Anchor, Option<AnchorResult>)]) -> Option<AnchorResult> {
        let mut current = 0;
        let mut verified = true;
        for (anchor, result) in claims {
            let consistent = matches!(
                result,
                Some(
                    AnchorResult::Matched
                        | AnchorResult::LostInterval
                        | AnchorResult::UnverifiedRestoreBoundary
                )
            );
            let superseded = consistent
                .then(|| self.explain(|restore| anchor.seal.sealed_at < restore.applied_at))
                .flatten();
            match superseded {
                None => current += 1,
                Some(AnchorResult::LostInterval) => {}
                Some(_) => verified = false,
            }
        }
        if current > 1 {
            Some(AnchorResult::AnchorConflict)
        } else if verified {
            None
        } else {
            Some(AnchorResult::UnverifiedRestoreBoundary)
        }
    }

    /// `lost_interval` when a covered restore satisfies `applies`,
    /// `unverified_restore_boundary` when only uncovered ones do, `None` when none does.
    fn explain(&self, applies: impl Fn(&RestoreRecord) -> bool) -> Option<AnchorResult> {
        let covered: Vec<bool> = self
            .restores
            .iter()
            .zip(&self.covered)
            .filter(|(restore, _)| applies(restore))
            .map(|(_, covered)| *covered)
            .collect();
        if covered.contains(&true) {
            Some(AnchorResult::LostInterval)
        } else if covered.is_empty() {
            None
        } else {
            Some(AnchorResult::UnverifiedRestoreBoundary)
        }
    }
}

fn restored_head(restore: &RestoreRecord, stream: Stream) -> i64 {
    restore
        .restored_heads
        .iter()
        .find(|head| head.stream == stream)
        .map_or(0, |head| head.seq)
}
