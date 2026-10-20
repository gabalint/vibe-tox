//! True concurrency for every §3.6 row (`docs/planning/m3-design-brief.md`,
//! NFR-01): the release and the change run together with `tokio::join!`,
//! [`ROUNDS`] times on fresh benches. Round `n` yields `n` times before
//! starting the change, so both commit orders occur. The audit sequence on the change's stream decides which committed first, and
//! that order's expectation from the table must hold. The race actor
//! prepares and releases; R, who keeps the releaser role, reads the outcome.
//! Synthetic data only.

use tokio::task::LocalSet;

use crate::m3_changes::Change;
use crate::m3_fixture::{Bench, Staff};
use crate::m3_release_races::{expect_refused, expect_released, released_events};
use crate::m3_report::action_of;

const ROUNDS: usize = 20;

/// Runs the rounds side by side on this thread; each has its own engine.
async fn race(change: Change) {
    let rounds = LocalSet::new();
    let handles: Vec<_> = (0..ROUNDS)
        .map(|round| rounds.spawn_local(race_once(change, round)))
        .collect();
    rounds
        .run_until(async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;
}

async fn race_once(change: Change, round: usize) {
    let bench = Bench::ready(change.variant()).await;
    bench.sign_in(Staff::Racer).await;
    let candidate = bench.prepare(None).await.unwrap();
    let prepared = change.prepare(&bench, Staff::Racer).await;
    bench.sign_in(Staff::Racer).await;
    let change_later = async {
        for _ in 0..round {
            tokio::task::yield_now().await;
        }
        prepared.commit(bench.lab().engine()).await
    };
    let (released, committed) = tokio::join!(bench.release(&candidate), change_later);
    let key = committed.unwrap();
    let reader = Staff::Releaser;
    if release_committed_first(&bench, change) {
        expect_released(&bench, &candidate, &released.unwrap(), key, reader).await;
    } else {
        let refused = released.unwrap_err();
        expect_refused(&bench, change, &candidate, &refused, reader).await;
    }
}

/// Whether the release's events precede the change's on the change's audit
/// stream; a release that did not commit did not come first.
fn release_committed_first(bench: &Bench, change: Change) -> bool {
    let Some((_, correlation)) = released_events(bench).pop() else {
        return false;
    };
    let release = bench
        .seq_of(change.stream(), &correlation)
        .expect("the release wrote no event on the change's stream");
    let (change_seq, _) = bench
        .events_of(change.stream(), action_of(change.command()), "succeeded")
        .pop()
        .unwrap();
    release < change_seq
}

#[tokio::test]
async fn release_races_qc_rejection() {
    race(Change::QcRejection).await;
}

#[tokio::test]
async fn release_races_a_deviation() {
    race(Change::Deviation).await;
}

#[tokio::test]
async fn release_races_an_identity_hold() {
    race(Change::IdentityHold).await;
}

#[tokio::test]
async fn release_races_a_cancellation() {
    race(Change::Cancellation).await;
}

#[tokio::test]
async fn release_races_a_result_revision() {
    race(Change::ResultRevision).await;
}

// The revocation of the releaser's own authority has no true-concurrency row: every
// raced command runs as one signed-in session, and nobody may change their own roles
// (security fix, 2026-10-13). The two sequential tests in `m3_release_races.rs` cover
// the row in both commit orders, with the quality manager revoking the releaser.

#[tokio::test]
async fn release_races_a_container_quality_hold() {
    race(Change::ContainerHold).await;
}

#[tokio::test]
async fn release_races_a_clarification_hold() {
    race(Change::ClarificationHold).await;
}

#[tokio::test]
async fn release_races_a_validity_hold() {
    race(Change::ValidityHold).await;
}
