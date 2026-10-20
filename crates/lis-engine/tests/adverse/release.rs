//! Release under a moving clock and an expiring session (REQ-RPT-001, NFR-01, NFR-19;
//! VAL-06, VAL-40, VAL-53). Expected outcomes: `docs/planning/m3-design-brief.md` §3
//! (release rechecks every gate in its transaction; staleness is a recomputed content
//! digest), `docs/planning/m4-design-brief.md` §6.1 (`clock` alert after a backward jump
//! or a clock behind the records) and the engine pipeline contract (a session idle past
//! the timeout is locked and every command but unlock is refused with `session_locked`).

use lis_analytical::{QcOutcome, QcScope};
use lis_auth::UnlockSessionInput;
use lis_core::ErrorCode;
use lis_engine::{AssessQcInput, GetRunQcInput, HealthState, IDLE_TIMEOUT};
use lis_ops::health::CheckCode;

use crate::fixture::{NEW_PASSWORD, secret, start_time};
use crate::m2_steps::gate_codes;
use crate::m3_fixture::{Bench, Staff, Variant};

/// The host clock steps back to the start of the day's records between preparing a
/// candidate and releasing it, and a QC rejection is committed in between with an
/// earlier timestamp than the candidate. Release must still see the rejection (gates are rechecked, not compared by
/// time), nothing is released, and health raises the clock alert.
#[tokio::test]
async fn a_clock_stepped_back_between_prepare_and_release_hides_no_gate() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Racer).await;
    let candidate = bench.prepare(None).await.unwrap();
    bench.lab().t().clock.set(start_time());

    // No one signs in after the step: TOTP refuses a code older than the last one used.
    reject_drug_qc(&bench).await;
    let refused = bench.release(&candidate).await.unwrap_err();

    assert_eq!(refused.code(), ErrorCode::Conflict, "{refused:?}");
    assert!(!gate_codes(&refused).is_empty(), "{refused:?}");
    assert_eq!(bench.count("rpt_revisions"), 0);
    let health = bench.lab().engine().get_host_health().await.unwrap();
    let clock_check = health
        .checks
        .iter()
        .find(|check| check.check == CheckCode::Clock)
        .unwrap();
    assert_eq!(clock_check.state, HealthState::Alert, "{clock_check:?}");
}

/// The releaser leaves the release dialog open past the idle timeout. The release is
/// refused with `session_locked`, the password typed into the dialog neither unlocks the
/// session nor releases anything, and after the same user unlocks, the unchanged
/// candidate (still within its 60 minutes) releases once.
#[tokio::test]
async fn a_release_after_the_idle_timeout_is_refused_until_the_same_user_unlocks() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Racer).await;
    let candidate = bench.prepare(None).await.unwrap();
    let t = bench.lab().t();
    t.advance(IDLE_TIMEOUT.as_secs() + 60);

    let refused = bench.release(&candidate).await.unwrap_err();

    assert_eq!(refused.code(), ErrorCode::SessionLocked, "{refused:?}");
    assert_eq!(bench.count("rpt_revisions"), 0);
    let e = bench.lab().engine();
    let unlocked = e
        .unlock_session(UnlockSessionInput {
            password: secret(NEW_PASSWORD),
        })
        .await
        .unwrap();
    assert!(!unlocked.locked);
    bench.release(&candidate).await.unwrap();
    assert_eq!(bench.count("rpt_revisions"), 1);
}

/// The signed-in race actor, a quality manager, rejects `SYN-MOR` QC on the bench run.
async fn reject_drug_qc(bench: &Bench) {
    let lab = bench.lab();
    let e = lab.engine();
    let qc = e
        .get_run_qc(GetRunQcInput { run_id: lab.run })
        .await
        .unwrap();
    let entry = qc
        .analytes
        .iter()
        .find(|a| a.analyte_id == lab.analytes.mor)
        .unwrap();
    e.assess_qc(AssessQcInput {
        scope: QcScope::RunAnalyte,
        run_id: Some(lab.run),
        analyte_id: Some(lab.analytes.mor),
        prep_batch_id: None,
        outcome: QcOutcome::Rejected,
        evidence_observation_ids: entry
            .observations
            .iter()
            .filter(|o| !o.superseded)
            .map(|o| o.observation_id)
            .collect(),
        supersedes_assessment_id: entry.assessments.current.as_ref().map(|a| a.assessment_id),
        reason: Some("Synthetic control out of range".to_owned()),
    })
    .await
    .unwrap();
}
