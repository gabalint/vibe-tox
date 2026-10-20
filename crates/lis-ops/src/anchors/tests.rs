//! Classification rules (M4 design brief §4 verification table) on constructed raw
//! statuses. The raw statuses themselves come from `lis_audit::compare_anchors`, which
//! `tests/anchors.rs` exercises on real databases.

use std::time::Duration;

use lis_audit::{Anchor, AnchorStatus, ChainLink, Seal, Stream};
use lis_core::{InstallationId, UtcTimestamp};

use super::classify::{AnchorResult, Classified, RestoreRecord, classify};

fn at(minutes: u64) -> UtcTimestamp {
    UtcTimestamp::parse("2026-10-13T08:00:00.000000000Z")
        .unwrap()
        .checked_add(Duration::from_secs(minutes * 60))
        .unwrap()
}

fn anchor(stream: Stream, seal_no: i64, last_seq: i64, sealed_minutes: u64) -> Anchor {
    Anchor {
        installation: "01900000-0000-7000-8000-0000000000aa"
            .parse::<InstallationId>()
            .unwrap(),
        seal: Seal {
            stream,
            seal_no,
            first_seq: 1,
            last_seq,
            prev_hash: "0".repeat(64),
            head_hash: format!("{last_seq:064x}"),
            sealed_at: at(sealed_minutes),
            hash_alg: "sha256-v1".to_owned(),
        },
    }
}

/// A restore applied at minute 60 that restored clinical events up to 10 and recorded
/// its own event as security event 20.
fn restore() -> RestoreRecord {
    RestoreRecord {
        applied_at: at(60),
        restored_heads: vec![ChainLink {
            stream: Stream::Clinical,
            seq: 10,
            hash: "0".repeat(64),
        }],
        applied_event: ChainLink {
            stream: Stream::Security,
            seq: 20,
            hash: "0".repeat(64),
        },
    }
}

/// A matched security anchor sealed after the restore, covering its event.
fn covering() -> Anchor {
    anchor(Stream::Security, 5, 21, 75)
}

fn results(classified: &[Classified]) -> Vec<(AnchorResult, i64)> {
    classified
        .iter()
        .map(|entry| (entry.result, entry.seal_no))
        .collect()
}

#[test]
fn an_anchor_lost_with_an_anchored_restore_is_a_lost_interval() {
    let anchors = [anchor(Stream::Clinical, 3, 15, 30), covering()];
    let statuses = [
        AnchorStatus::LocalEventsMissing { anchor: 0 },
        AnchorStatus::Matched { anchor: 1 },
    ];
    assert_eq!(
        results(&classify(&anchors, &statuses, &[restore()])),
        [(AnchorResult::LostInterval, 3), (AnchorResult::Matched, 5)]
    );
    let mismatch = [
        AnchorStatus::AnchorMismatch { anchor: 0 },
        statuses[1].clone(),
    ];
    assert_eq!(
        classify(&anchors, &mismatch, &[restore()])[0].result,
        AnchorResult::LostInterval
    );
}

#[test]
fn a_restore_whose_event_is_not_anchored_explains_nothing_for_certain() {
    let anchors = [
        anchor(Stream::Clinical, 3, 15, 30),
        anchor(Stream::Security, 4, 19, 50),
    ];
    let statuses = [
        AnchorStatus::LocalEventsMissing { anchor: 0 },
        AnchorStatus::Matched { anchor: 1 },
    ];
    let classified = classify(&anchors, &statuses, &[restore()]);
    assert_eq!(
        classified[0].result,
        AnchorResult::UnverifiedRestoreBoundary
    );
    assert!(classified[0].result.is_problem());
}

#[test]
fn findings_a_restore_does_not_explain_stand() {
    let anchors = [
        anchor(Stream::Clinical, 3, 15, 61),
        anchor(Stream::Clinical, 2, 9, 30),
        anchor(Stream::Access, 1, 4, 30),
        covering(),
    ];
    let statuses = [
        AnchorStatus::LocalEventsMissing { anchor: 0 },
        AnchorStatus::AnchorMismatch { anchor: 1 },
        AnchorStatus::LocalEventsMissing { anchor: 2 },
        AnchorStatus::Matched { anchor: 3 },
    ];
    assert_eq!(
        results(&classify(&anchors, &statuses, &[restore()])),
        [
            (AnchorResult::LocalEventsMissing, 3),
            (AnchorResult::AnchorMismatch, 2),
            (AnchorResult::LostInterval, 1),
            (AnchorResult::Matched, 5),
        ],
        "sealed after the restore; within the restored head; a stream the set did not \
         hold was restored empty"
    );
    assert_eq!(
        results(&classify(&anchors[..1], &statuses[..1], &[])),
        [(AnchorResult::LocalEventsMissing, 3)]
    );
}

#[test]
fn a_reissued_seal_number_is_explained_by_an_anchored_restore() {
    let anchors = [
        anchor(Stream::Clinical, 3, 15, 30),
        anchor(Stream::Clinical, 3, 12, 70),
        covering(),
    ];
    let statuses = [
        AnchorStatus::LocalEventsMissing { anchor: 0 },
        AnchorStatus::Matched { anchor: 1 },
        AnchorStatus::Matched { anchor: 2 },
        AnchorStatus::AnchorConflict {
            stream: Stream::Clinical,
            seal_no: 3,
        },
    ];
    assert_eq!(
        results(&classify(&anchors, &statuses, &[restore()])),
        [
            (AnchorResult::LostInterval, 3),
            (AnchorResult::Matched, 3),
            (AnchorResult::Matched, 5),
        ]
    );
    let unanchored = &statuses[..2]
        .iter()
        .cloned()
        .chain([statuses[3].clone()])
        .collect::<Vec<_>>();
    assert_eq!(
        results(&classify(&anchors[..2], unanchored, &[restore()])),
        [
            (AnchorResult::UnverifiedRestoreBoundary, 3),
            (AnchorResult::Matched, 3),
            (AnchorResult::UnverifiedRestoreBoundary, 3),
        ]
    );
}

#[test]
fn conflicts_no_restore_explains_stand() {
    let conflict = AnchorStatus::AnchorConflict {
        stream: Stream::Clinical,
        seal_no: 3,
    };
    let after_restore = [
        anchor(Stream::Clinical, 3, 15, 65),
        anchor(Stream::Clinical, 3, 12, 70),
        covering(),
    ];
    let statuses = [
        AnchorStatus::AnchorMismatch { anchor: 0 },
        AnchorStatus::Matched { anchor: 1 },
        AnchorStatus::Matched { anchor: 2 },
        conflict,
    ];
    let classified = classify(&after_restore, &statuses, &[restore()]);
    assert_eq!(classified[3].result, AnchorResult::AnchorConflict);
    let without_restore = classify(&after_restore, &statuses, &[]);
    assert_eq!(without_restore[3].result, AnchorResult::AnchorConflict);
}

#[test]
fn gaps_and_unanchored_ranges_pass_through() {
    let statuses = [
        AnchorStatus::AnchorGap {
            stream: Stream::Security,
            first_seal_no: 2,
            last_seal_no: 4,
        },
        AnchorStatus::Unanchored {
            stream: Stream::Access,
            first_seal_no: 7,
            last_seal_no: 9,
        },
    ];
    let classified = classify(&[], &statuses, &[restore()]);
    assert_eq!(
        classified
            .iter()
            .map(|entry| (
                entry.result,
                entry.stream,
                entry.seal_no,
                entry.last_seal_no
            ))
            .collect::<Vec<_>>(),
        [
            (AnchorResult::AnchorGap, Stream::Security, 2, 4),
            (AnchorResult::Unanchored, Stream::Access, 7, 9),
        ]
    );
}

#[test]
fn only_expected_results_are_not_problems() {
    let calm = [
        AnchorResult::Matched,
        AnchorResult::LostInterval,
        AnchorResult::Unanchored,
    ];
    for result in [
        AnchorResult::Matched,
        AnchorResult::LostInterval,
        AnchorResult::UnverifiedRestoreBoundary,
        AnchorResult::LocalEventsMissing,
        AnchorResult::AnchorMismatch,
        AnchorResult::AnchorGap,
        AnchorResult::AnchorConflict,
        AnchorResult::Unanchored,
    ] {
        assert_eq!(
            result.is_problem(),
            !calm.contains(&result),
            "{}",
            result.as_str()
        );
    }
}
