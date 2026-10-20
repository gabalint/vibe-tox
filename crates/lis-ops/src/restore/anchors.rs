//! Comparing a staged database with the anchors on target folders (M4 design brief §3.6
//! step 5, §4). The anchors area reads the files and classifies each anchor against the
//! restored chains, explaining earlier restores from the restored database's own restore
//! records ([`crate::anchors::verify_anchors`]). Staging adds one rule of its own: an
//! anchor sealed after the staged set was taken that names events beyond the set's heads
//! is the set's `lost_interval`, expected and counted; one sealed before the set was
//! taken stays `local_events_missing`, because the set then lacks events it should hold.

use lis_audit::{Seal, Stream};
use lis_core::{InstallationId, UtcTimestamp};
use rusqlite::{Connection, Row};

use super::findings::{FindingCode, Findings};
use super::record::restore_records;
use crate::anchors::{AnchorError, AnchorSummary, ReadAnchors, verify_anchors};
use crate::backup::BackupError;

/// Number of anchor counts stored per restore and restore test.
pub(crate) const ANCHOR_COUNTS: usize = 9;

/// What the staged set holds, for explaining anchors.
pub(crate) struct SetPosition {
    pub(crate) installation: InstallationId,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) heads: Vec<(Stream, i64)>,
}

/// The comparison's summary, findings, and the newest lost-interval seal time.
pub(crate) struct AnchorCheck {
    pub(crate) summary: AnchorSummary,
    pub(crate) findings: Findings,
    pub(crate) lost_interval_until: Option<UtcTimestamp>,
}

/// Compares the anchors `read` from the target folders with the staged database `conn`.
/// Invalid lines and unreadable files count as mismatches: a staged set passes only when
/// every anchor was read and is explained.
pub(crate) fn compare(
    conn: &Connection,
    set: &SetPosition,
    read: &ReadAnchors,
) -> Result<AnchorCheck, BackupError> {
    let restores = restore_records(conn)?;
    let verification = verify_anchors(conn, read, &restores).map_err(backup_error)?;
    let mut summary = verification.summary;
    let mut lost_interval_until = None;
    for finding in &verification.findings {
        let claimants = claimants(read, set, finding.stream.as_deref(), finding.seal_no);
        let after_set = !claimants.is_empty()
            && claimants.iter().all(|seal| {
                // A seal covering events beyond the set's heads was made after the
                // snapshot even when the clock shows the same instant.
                seal.sealed_at >= set.created_at && seal.last_seq > head(&set.heads, seal.stream)
            });
        let moved = match finding.code.as_str() {
            "local_events_missing" if after_set => Some(&mut summary.local_events_missing),
            "unverified_restore_boundary" if after_set => {
                Some(&mut summary.unverified_restore_boundary)
            }
            "lost_interval" => None,
            _ => continue,
        };
        if let Some(counter) = moved {
            *counter = counter.saturating_sub(1);
            summary.lost_interval += 1;
        }
        let newest = claimants.iter().map(|seal| seal.sealed_at).max();
        lost_interval_until = lost_interval_until.max(newest);
    }
    let mut findings = Findings::default();
    let unexplained = summary.mismatched + summary.conflicts + summary.unverified_restore_boundary;
    findings.add(
        FindingCode::AnchorMismatch,
        unexplained + read.invalid_lines + read.unreadable,
    );
    findings.add(
        FindingCode::LocalEventsMissing,
        summary.local_events_missing,
    );
    findings.add(FindingCode::LostInterval, summary.lost_interval);
    findings.add(FindingCode::AnchorGap, summary.gaps);
    Ok(AnchorCheck {
        summary,
        findings,
        lost_interval_until,
    })
}

/// The seals of the set installation's anchors for `(stream, seal_no)`.
fn claimants<'a>(
    read: &'a ReadAnchors,
    set: &SetPosition,
    stream: Option<&str>,
    seal_no: Option<i64>,
) -> Vec<&'a Seal> {
    let (Some(stream), Some(seal_no)) = (stream.and_then(super::stream_named), seal_no) else {
        return Vec::new();
    };
    read.anchors
        .iter()
        .filter(|anchor| {
            anchor.installation == set.installation
                && anchor.seal.stream == stream
                && anchor.seal.seal_no == seal_no
        })
        .map(|anchor| &anchor.seal)
        .collect()
}

fn head(heads: &[(Stream, i64)], stream: Stream) -> i64 {
    heads
        .iter()
        .find(|(head_stream, _)| *head_stream == stream)
        .map_or(0, |(_, seq)| *seq)
}

fn backup_error(error: AnchorError) -> BackupError {
    match error {
        AnchorError::Targets(error) => error,
        AnchorError::Store(error) => BackupError::Store(error),
        AnchorError::NoInstallation => BackupError::NoInstallation,
        AnchorError::NoTargetAvailable | AnchorError::FolderUnreadable => {
            BackupError::Io(std::io::Error::other("anchor folder unreadable"))
        }
    }
}

/// The counts in their stored order: compared, matched, lost interval, unverified
/// restore boundary, mismatched, local events missing, conflicts, gaps, unanchored.
pub(crate) fn counts(summary: &AnchorSummary) -> [u64; ANCHOR_COUNTS] {
    [
        summary.compared,
        summary.matched,
        summary.lost_interval,
        summary.unverified_restore_boundary,
        summary.mismatched,
        summary.local_events_missing,
        summary.conflicts,
        summary.gaps,
        summary.unanchored,
    ]
}

/// The summary of counts in [`counts`] order.
pub(crate) fn from_counts(counts: [u64; ANCHOR_COUNTS]) -> AnchorSummary {
    let [
        compared,
        matched,
        lost_interval,
        unverified_restore_boundary,
        mismatched,
        local_events_missing,
        conflicts,
        gaps,
        unanchored,
    ] = counts;
    AnchorSummary {
        compared,
        matched,
        lost_interval,
        unverified_restore_boundary,
        mismatched,
        local_events_missing,
        conflicts,
        gaps,
        unanchored,
    }
}

/// The summary stored in [`ANCHOR_COUNTS`] columns from `first` on, in [`counts`] order;
/// `None` when they are NULL.
pub(crate) fn summary_at(row: &Row<'_>, first: usize) -> rusqlite::Result<Option<AnchorSummary>> {
    let mut values = [0_u64; ANCHOR_COUNTS];
    for (offset, value) in values.iter_mut().enumerate() {
        match row.get::<_, Option<i64>>(first + offset)? {
            Some(stored) => *value = u64::try_from(stored).unwrap_or(0),
            None => return Ok(None),
        }
    }
    Ok(Some(from_counts(values)))
}
