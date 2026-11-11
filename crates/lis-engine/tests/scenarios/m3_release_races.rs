//! Release versus each change of `docs/planning/m3-design-brief.md` §3.6,
//! in both commit orders (NFR-01, REQ-RPT-001/002, REQ-IDN-004). The
//! expected outcomes are the table's; [`expect_refused`] and
//! [`expect_released`] are shared with the true-concurrency suite
//! ([`crate::m3_concurrency`]). Synthetic data only.

use lis_core::{Clock, CommandError, ErrorCode};
use lis_engine::{
    CandidateView, CloseDeviationInput, RecordImpactAssessmentInput, RevisionSummary,
};
use lis_reporting::{CandidateState, ImpactDecision};
use lis_store::{ArtifactStore, MAX_ARTIFACT_BYTES};

use crate::fixture::start_time;
use crate::m2_lab::Role;
use crate::m2_steps::gate_codes;
use crate::m3_changes::{Change, Prepared, Refusal};
use crate::m3_fixture::{Bench, Staff};
use crate::m3_report::{action_of, open_keys};

/// X first: R prepares C, X commits, then R releases C.
async fn change_first(change: Change) {
    let bench = Bench::ready(change.variant()).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let prepared = change.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(change.actor()).await;
    prepared.commit(bench.lab().engine()).await.unwrap();
    bench.sign_in(Staff::Releaser).await;
    let refused = bench.release(&candidate).await.unwrap_err();
    expect_refused(&bench, change, &candidate, &refused, Staff::Racer).await;
}

/// Release first: R releases C as revision 1 with obligation O1, then X
/// commits; revision 1's row and bytes do not change.
async fn release_first(change: Change) {
    let bench = Bench::ready(change.variant()).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let revision = bench.release(&candidate).await.unwrap();
    let row = bench.revision_row(revision.revision_id);
    let bytes = verified_artifact(&bench, &revision.artifact_sha256);
    let prepared = change.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(change.actor()).await;
    let key = prepared.commit(bench.lab().engine()).await.unwrap();
    assert_eq!(bench.revision_row(revision.revision_id), row);
    assert_eq!(verified_artifact(&bench, &revision.artifact_sha256), bytes);
    expect_released(&bench, &candidate, &revision, key, Staff::Racer).await;
}

/// "X first: release fails with" the row's refusal; afterwards there is no
/// revision, no obligation and no `report.release` event, and C stays
/// `ready` (reading as stale when its content changed). `reader` is a
/// releaser the change leaves in place. The row's exact reason code is
/// checked last, after every safety outcome.
pub(crate) async fn expect_refused(
    bench: &Bench,
    change: Change,
    candidate: &CandidateView,
    refused: &CommandError,
    reader: Staff,
) {
    let reason = expect_refusal(bench, change, refused);
    assert_eq!(bench.count("rpt_revisions"), 0);
    assert_eq!(bench.count("rpt_delivery_obligations"), 0);
    assert!(released_events(bench).is_empty());
    let read = bench.candidate(reader, candidate).await;
    assert_eq!(read.state, CandidateState::Ready);
    match change.refusal() {
        Refusal::Stale(_) => assert!(read.stale),
        Refusal::Blocked(_) | Refusal::Forbidden => assert!(!read.stale),
        Refusal::Gate(_) => {}
    }
    if let Some(reason) = reason {
        let codes = gate_codes(refused);
        assert!(
            codes.iter().any(|c| c == reason),
            "{reason} not in {codes:?}"
        );
    }
}

/// The row's error code and side event; returns the gate reason the row
/// names, if any.
fn expect_refusal(bench: &Bench, change: Change, refused: &CommandError) -> Option<&'static str> {
    let code = refused.code();
    match change.refusal() {
        Refusal::Stale(reason) => {
            assert_eq!(code, ErrorCode::Conflict, "{refused:?}");
            assert!(gate_codes(refused).contains(&"candidate_stale".to_owned()));
            Some(reason)
        }
        Refusal::Blocked(reason) => {
            assert_eq!(code, ErrorCode::GateBlocked, "{refused:?}");
            Some(reason)
        }
        Refusal::Gate(reason) => {
            let refusal = matches!(code, ErrorCode::Conflict | ErrorCode::GateBlocked);
            assert!(refusal, "{refused:?}");
            Some(reason)
        }
        Refusal::Forbidden => {
            assert_eq!(code, ErrorCode::Forbidden, "{refused:?}");
            let denied = bench.events_of("security", action_of("release_report"), "denied");
            assert_eq!(denied.len(), 1);
            None
        }
    }
}

/// "Release first": revision 1 holds C's content and bytes, then
/// [`expect_impact`]. `reader` is a releaser the change leaves in place;
/// they also attempt delivery.
pub(crate) async fn expect_released(
    bench: &Bench,
    candidate: &CandidateView,
    revision: &RevisionSummary,
    key: Option<String>,
    reader: Staff,
) {
    assert_eq!(revision.revision_no, 1);
    let artifact = candidate.artifact_sha256.as_deref();
    assert_eq!(Some(revision.artifact_sha256.as_str()), artifact);
    let row = bench.revision_row(revision.revision_id).concat();
    assert!(row.contains(&candidate.content_sha256), "{row}");
    verified_artifact(bench, &revision.artifact_sha256);
    expect_impact(bench, revision, key, reader).await;
}

/// The impact queue lists revision 1 with the row's key, and delivery of O1
/// is refused with `revision_impact_open` until Q records `no_impact`. With
/// no key (revocation is prospective) nothing is open and the queue stays
/// empty. The exact key is checked last, after every safety outcome.
async fn expect_impact(
    bench: &Bench,
    revision: &RevisionSummary,
    key: Option<String>,
    reader: Staff,
) {
    let queue = bench.impact_queue().await;
    let report = bench.report(reader).await;
    assert_eq!(report.obligations.len(), 1);
    let keys = open_keys(&report);
    let Some(key) = key else {
        assert!(keys.is_empty() && queue.is_empty(), "{keys:?} {queue:?}");
        return;
    };
    assert!(!keys.is_empty(), "no impact opened for {key}");
    let listed = queue.contains(&revision.revision_id.to_string());
    assert!(listed, "{queue:?}");
    let obligation = report.obligations[0].obligation_id;
    let paused = bench.hand_off(obligation).await.unwrap_err();
    assert!(gate_codes(&paused).contains(&"revision_impact_open".to_owned()));
    bench
        .assess_no_impact(revision.revision_id, keys.clone())
        .await;
    bench.sign_in(reader).await;
    bench.hand_off(obligation).await.unwrap();
    assert!(keys.contains(&key), "{key} not in {keys:?}");
}

/// `(seq, correlation ID)` of every succeeded `report.release` event.
pub(crate) fn released_events(bench: &Bench) -> Vec<(i64, String)> {
    bench.events_of("clinical", action_of("release_report"), "succeeded")
}

/// The stored bytes of `sha256`, read through the store's digest check.
pub(crate) fn verified_artifact(bench: &Bench, sha256: &str) -> Vec<u8> {
    let path = bench.artifact_path(sha256);
    let store = ArtifactStore::open(path.parent().unwrap(), MAX_ARTIFACT_BYTES).unwrap();
    store.read(&sha256.parse().unwrap()).unwrap()
}

#[tokio::test]
async fn release_fails_when_qc_rejection_commits_first() {
    change_first(Change::QcRejection).await;
}

#[tokio::test]
async fn qc_rejection_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::QcRejection).await;
}

#[tokio::test]
async fn release_fails_when_a_deviation_opens_first() {
    change_first(Change::Deviation).await;
}

#[tokio::test]
async fn a_deviation_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::Deviation).await;
}

#[tokio::test]
async fn release_is_blocked_when_an_identity_hold_opens_first() {
    change_first(Change::IdentityHold).await;
}

#[tokio::test]
async fn an_identity_hold_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::IdentityHold).await;
}

#[tokio::test]
async fn release_fails_when_the_reported_item_is_cancelled_first() {
    change_first(Change::Cancellation).await;
}

#[tokio::test]
async fn a_cancellation_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::Cancellation).await;
}

#[tokio::test]
async fn release_fails_when_a_result_revision_commits_first() {
    change_first(Change::ResultRevision).await;
}

#[tokio::test]
async fn a_result_revision_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::ResultRevision).await;
}

#[tokio::test]
async fn release_is_forbidden_when_the_releaser_role_is_revoked_first() {
    change_first(Change::ReleaserAuthority).await;
}

#[tokio::test]
async fn revoking_the_releaser_after_release_opens_no_impact() {
    release_first(Change::ReleaserAuthority).await;
}

#[tokio::test]
async fn release_fails_when_a_container_quality_hold_is_placed_first() {
    change_first(Change::ContainerHold).await;
}

#[tokio::test]
async fn a_container_hold_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::ContainerHold).await;
}

#[tokio::test]
async fn release_fails_when_a_clarification_hold_is_placed_first() {
    change_first(Change::ClarificationHold).await;
}

#[tokio::test]
async fn a_clarification_hold_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::ClarificationHold).await;
}

#[tokio::test]
async fn release_fails_when_validity_is_reassessed_as_hold_first() {
    change_first(Change::ValidityHold).await;
}

#[tokio::test]
async fn a_validity_hold_after_release_opens_impact_and_pauses_delivery() {
    release_first(Change::ValidityHold).await;
}

/// Review recovery is prospective: it never erases the deviation that
/// invalidated an already released line (review finding 3, scenario A).
#[tokio::test]
async fn a_closed_deviation_and_rereview_do_not_erase_released_impact() {
    let bench = Bench::ready(crate::m3_fixture::Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let released = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let Prepared::Deviation(input, revision) =
        Change::Deviation.prepare(&bench, Staff::Releaser).await
    else {
        unreachable!();
    };
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    let deviation = bench.lab().engine().open_deviation(input).await.unwrap();
    let key = format!(
        "review_stale:{revision}:deviation_open:{}",
        deviation.deviation_id
    );
    bench
        .lab()
        .engine()
        .close_deviation(CloseDeviationInput {
            deviation_id: deviation.deviation_id,
            expected_revision: deviation.revision,
            disposition: "Synthetic cause corrected".to_owned(),
            reason: "Synthetic recovery".to_owned(),
        })
        .await
        .unwrap();
    bench.desk().await.approve_all(bench.index).await.unwrap();
    let report = bench.report(Staff::Releaser).await;
    assert!(open_keys(&report).contains(&key));
    assert!(
        bench
            .impact_queue()
            .await
            .contains(&released.revision_id.to_string())
    );
}

/// A second causal record gets a second key; an assessment of the first
/// cannot cover it (review finding 3, scenario B).
#[tokio::test]
async fn a_second_deviation_reopens_impact_with_its_own_key() {
    let bench = Bench::ready(crate::m3_fixture::Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let released = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let first = Change::Deviation.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    let first_key = first.commit(bench.lab().engine()).await.unwrap().unwrap();
    bench
        .assess_no_impact(released.revision_id, vec![first_key.clone()])
        .await;
    let second = Change::Deviation.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    let second_key = second.commit(bench.lab().engine()).await.unwrap().unwrap();
    let keys = open_keys(&bench.report(Staff::Releaser).await);
    assert_ne!(first_key, second_key);
    assert!(!keys.contains(&first_key));
    assert!(keys.contains(&second_key));
}

/// Commit sequence, not the host clock, decides whether a hold followed the
/// release (review finding 4).
#[tokio::test]
async fn a_backward_clock_step_does_not_hide_post_release_holds() {
    for change in [
        Change::IdentityHold,
        Change::ContainerHold,
        Change::ClarificationHold,
    ] {
        let bench = Bench::ready(crate::m3_fixture::Variant::Panel).await;
        bench.sign_in(Staff::Releaser).await;
        let released = bench
            .release(&bench.prepare(None).await.unwrap())
            .await
            .unwrap();
        let prepared = change.prepare(&bench, Staff::Releaser).await;
        bench.sign_in(change.actor()).await;
        let current_time = bench.lab().t().clock.now();
        bench.lab().t().clock.set(start_time());
        let key = prepared
            .commit(bench.lab().engine())
            .await
            .unwrap()
            .unwrap();
        bench.lab().t().clock.set(current_time);
        assert!(open_keys(&bench.report(Staff::Releaser).await).contains(&key));
        assert!(
            bench
                .impact_queue()
                .await
                .contains(&released.revision_id.to_string())
        );
    }
}

/// Once quality requires a correction, a later no-impact decision on a new
/// cause cannot resume delivery of that revision (review finding 2).
#[tokio::test]
async fn a_later_no_impact_assessment_does_not_clear_correction_due() {
    let bench = Bench::ready(crate::m3_fixture::Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let released = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let first = Change::ContainerHold.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    let first_key = first.commit(bench.lab().engine()).await.unwrap().unwrap();
    bench
        .lab()
        .engine()
        .record_impact_assessment(RecordImpactAssessmentInput {
            revision_id: released.revision_id,
            keys: vec![first_key],
            decision: ImpactDecision::CorrectionRequired,
            reason: "Synthetic correction required".to_owned(),
        })
        .await
        .unwrap();
    let second = Change::Deviation.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    let second_key = second.commit(bench.lab().engine()).await.unwrap().unwrap();
    bench
        .assess_no_impact(released.revision_id, vec![second_key])
        .await;
    let report = bench.report(Staff::Releaser).await;
    let impact = report.impact.unwrap();
    assert!(impact.correction_due);
    assert!(impact.open.is_empty());
    let obligation = report.obligations[0].obligation_id;
    assert!(
        gate_codes(&bench.hand_off(obligation).await.unwrap_err())
            .contains(&"revision_impact_open".to_owned())
    );
}
