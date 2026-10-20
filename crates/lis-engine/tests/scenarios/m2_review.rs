//! M2 technical-review gates through the engine (REQ-RES-005,
//! REQ-ANA-004/005, REQ-ORD-002, REQ-SPC-005, NFR-01): each gate refuses
//! approval, both commit orders of a QC rejection and a review are safe, and
//! staleness is computed, never written. Expected outcomes from
//! `docs/planning/m2-design-brief.md` §2.4, §2.5 and §3.6. Synthetic data only.

use lis_analytical::AttemptKind;
use lis_analytical::{QcOutcome, RevisionIndicator};
use lis_core::{CommandError, ErrorCode};
use lis_engine::{
    AddPatientPositionInput, CloseDeviationInput, CloseRunInput, CreateRunInput, DeviationTarget,
    GetOrderInput, GetRunInput, ListValidityPoliciesInput, OpenDeviationInput,
    PlaceClarificationHoldInput, PlaceContainerHoldInput, RecordValidityAssessmentInput,
    ReviewView,
};
use lis_results::{ResultRevisionId, ReviewStatus};
use lis_specimens::ContainerHoldKind;
use lis_specimens::SpecimenId;

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{JOURNEY, gate_codes, line, message};

/// Asserts a gate refusal that names `code`.
pub(crate) fn assert_refused(result: Result<ReviewView, CommandError>, code: &str) {
    let error = result.unwrap_err();
    assert_eq!(error.code(), ErrorCode::GateBlocked, "{error:?}");
    let codes = gate_codes(&error);
    assert!(codes.iter().any(|c| c == code), "{code} not in {codes:?}");
}

/// The current revision of the `code` line of sample `index`.
pub(crate) async fn revision(lab: &Lab, index: usize, code: &str) -> ResultRevisionId {
    let results = lab.results(index).await;
    line(&results, code).current.as_ref().unwrap().revision_id
}

/// The stale reasons of the `code` line of sample `index`.
async fn stale_reasons(lab: &Lab, index: usize, code: &str) -> Vec<String> {
    match line(&lab.results(index).await, code).review_status.clone() {
        ReviewStatus::Stale { reasons } => reasons.into_iter().map(|r| r.code).collect(),
        other => panic!("{code} of sample {index} is not stale: {other:?}"),
    }
}

/// Brief §2.4: no assessment is pending; a rejection committed before the
/// review refuses it.
#[tokio::test]
async fn review_is_refused_while_qc_is_pending_or_after_a_committed_rejection() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    let mor = revision(&lab, 0, "SYN-MOR").await;
    assert_refused(lab.approve(0, vec![mor]).await, "qc_pending");
    lab.assess_qc(lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    assert_refused(lab.approve(0, vec![mor]).await, "qc_rejected");
    // REQ-ANA-005: no waiver; a rejection is never relabeled as accepted.
    let relabel = lab.assess_qc(lab.analytes.mor, QcOutcome::Accepted).await;
    assert_eq!(relabel.unwrap_err().code(), ErrorCode::Conflict);
    assert_refused(lab.approve(0, vec![mor]).await, "qc_rejected");
}

/// Brief §2.5: a rejection committed after the review leaves the review
/// row unchanged and the computed status becomes stale.
#[tokio::test]
async fn a_qc_rejection_committed_after_review_makes_the_review_stale() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let review = lab.approve_all(0).await.unwrap();
    lab.assess_qc(lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    assert!(
        stale_reasons(&lab, 0, "SYN-MOR")
            .await
            .contains(&"qc_rejected".to_owned())
    );
    let oxy = line(&lab.results(0).await, "SYN-OXY").review_status.clone();
    assert_eq!(
        oxy,
        ReviewStatus::Current {
            review_id: review.review_id
        }
    );
    let mor = revision(&lab, 0, "SYN-MOR").await;
    assert_refused(lab.approve(0, vec![mor]).await, "qc_rejected");
}

/// Brief §3.5: a corrected QC row is a new observation, so the run × analyte
/// assessment is `qc_stale` and so is the review that relied on it.
#[tokio::test]
async fn new_qc_data_after_review_makes_qc_and_review_stale() {
    let lab = Lab::start(RevisionIndicator::ResultStatusCorrection).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.approve_all(0).await.unwrap();
    let corrected = lab
        .import(&message(&[
            "P|1",
            "O|1|QC-LOW-SYN|SYN-POS-01||R",
            "R|1|^^^SYN-MOR|37.9|ng/mL||N||C||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
        ]))
        .await;
    assert_eq!((corrected.rows_qc, corrected.rows_new), (1, 0));
    assert!(
        stale_reasons(&lab, 0, "SYN-MOR")
            .await
            .contains(&"qc_stale".to_owned())
    );
    let mor = revision(&lab, 1, "SYN-MOR").await;
    assert_refused(lab.approve(1, vec![mor]).await, "qc_stale");
    // Reassessing on the current observations clears the gate.
    lab.assess_qc(lab.analytes.mor, QcOutcome::Accepted)
        .await
        .unwrap();
    lab.approve(1, vec![mor]).await.unwrap();
}

/// REQ-ANA-004: a deviation scoped to the run and one analyte blocks only
/// that analyte's review, until it is closed.
#[tokio::test]
async fn an_open_deviation_blocks_review_of_its_analyte_until_closed() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.sign_in(Role::Quality).await;
    let deviation = lab
        .engine()
        .open_deviation(OpenDeviationInput {
            scope: DeviationTarget::Run(lab.run),
            analyte_id: Some(lab.analytes.mor),
            description: "Synthetic internal standard drift".to_owned(),
            reason: "Synthetic bench observation".to_owned(),
        })
        .await
        .unwrap();
    let mor = revision(&lab, 0, "SYN-MOR").await;
    let oxy = revision(&lab, 0, "SYN-OXY").await;
    assert_refused(lab.approve(0, vec![mor]).await, "deviation_open");
    lab.approve(0, vec![oxy]).await.unwrap();
    lab.sign_in(Role::Quality).await;
    lab.engine()
        .close_deviation(CloseDeviationInput {
            deviation_id: deviation.deviation_id,
            expected_revision: deviation.revision,
            disposition: "Synthetic results unaffected".to_owned(),
            reason: "Synthetic investigation closed".to_owned(),
        })
        .await
        .unwrap();
    lab.approve(0, vec![mor]).await.unwrap();
}

/// Brief §2.5: `spc::testing_gate` and `ord::testing_gate` are rechecked at
/// approval: a container hold or an order clarification hold refuses it.
#[tokio::test]
async fn specimen_and_order_holds_refuse_review() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.sign_in(Role::Quality).await;
    lab.engine()
        .place_hold(PlaceContainerHoldInput {
            container_id: lab.samples[1].container_id,
            kind: ContainerHoldKind::Quality,
            reason: "Synthetic leaking container".to_owned(),
        })
        .await
        .unwrap();
    assert_refused(lab.approve_all(1).await, "container_on_hold");

    lab.sign_in(Role::Accessioner).await;
    let order_id = lab.samples[2].order_id;
    let order = lab
        .engine()
        .get_order(GetOrderInput { order_id })
        .await
        .unwrap();
    lab.engine()
        .place_clarification_hold(PlaceClarificationHoldInput {
            order_id,
            expected_revision: order.revision,
            deficiency: lis_orders::ClarificationDeficiency::OtherAuthority,
            note: Some("Synthetic unclear requisition".to_owned()),
        })
        .await
        .unwrap();
    assert_refused(lab.approve_all(2).await, "clarification_hold_open");
    lab.approve_all(0).await.unwrap();
}

/// Brief §1 (SPC-004) and §2.5: no drug line of a specimen whose order has
/// a validity component passes review before a current validity assessment
/// exists; specimens without one are not affected.
#[tokio::test]
async fn drug_review_waits_for_a_validity_assessment_where_validity_applies() {
    let mut lab = Lab::start(RevisionIndicator::None).await;
    let sample = lab.add_sample(lab.orderables.with_validity.1).await;
    lab.place(&sample).await;
    let label = sample.label_code.clone();
    lab.samples.push(sample);
    lab.import(JOURNEY).await;
    lab.import(&message(&[
        "P|1",
        &format!("O|1|{label}|SYN-POS-11||R"),
        "R|1|^^^SYN-MOR|60.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|10.0|ng/mL||N||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
    ]))
    .await;
    lab.accept_qc().await;
    let results = lab.results(5).await;
    assert!(results.validity_required);
    let gates = &line(&results, "SYN-MOR").gates;
    assert!(
        gates
            .iter()
            .any(|g| g.code == "validity_assessment_missing"),
        "{gates:?}"
    );
    let drugs = vec![
        revision(&lab, 5, "SYN-MOR").await,
        revision(&lab, 5, "SYN-OXY").await,
    ];
    assert_refused(
        lab.approve(5, drugs.clone()).await,
        "validity_assessment_missing",
    );
    lab.approve_all(0).await.unwrap();

    let creatinine = validity_result(&lab, &label).await;
    let specimen_id = lab.samples[5].specimen_id;
    // Only validity-purpose results count as validity evidence.
    let wrong = assess_validity(&lab, specimen_id, drugs[0]).await;
    assert_eq!(wrong.unwrap_err().code(), ErrorCode::Conflict);
    assess_validity(&lab, specimen_id, creatinine)
        .await
        .unwrap();
    lab.approve(5, drugs).await.unwrap();
}

/// Measures the validity analyte on its own run: the drug run is closed
/// first, so the label matches one open position.
async fn validity_result(lab: &Lab, label: &str) -> ResultRevisionId {
    lab.sign_in(Role::Analyst).await;
    let e = lab.engine();
    let drugs = e.get_run(GetRunInput { run_id: lab.run }).await.unwrap();
    e.close_run(CloseRunInput {
        run_id: lab.run,
        expected_revision: drugs.revision,
        reason: None,
    })
    .await
    .unwrap();
    let run = e
        .create_run(CreateRunInput {
            instrument_id: lab.instrument,
            method_version_id: lab.methods.validity,
            lot_ids: Vec::new(),
        })
        .await
        .unwrap();
    e.add_patient_position(AddPatientPositionInput {
        run_id: run.run_id,
        position_label: "V1".to_owned(),
        container_label: label.to_owned(),
        attempt_kind: AttemptKind::Initial,
        related_execution_id: None,
        dilution_factor: None,
        prep_batch_id: None,
        reason: None,
    })
    .await
    .unwrap();
    lab.import(&message(&[
        "P|1",
        &format!("O|1|{label}|SYN-POS-12||R"),
        "R|1|^^^SYN-CRE|120.0|mg/dL||N||F||SYN-TECH-01||20261012140500|SYNTH-LCMS-01",
    ]))
    .await;
    revision(lab, 5, "SYN-CRE").await
}

async fn assess_validity(
    lab: &Lab,
    specimen_id: SpecimenId,
    evidence: ResultRevisionId,
) -> Result<lis_engine::ValidityAssessmentView, CommandError> {
    lab.sign_in(Role::Reviewer).await;
    let policies = lab
        .engine()
        .list_validity_policies(ListValidityPoliciesInput {
            specimen_type_code: "SYN-UR".to_owned(),
        })
        .await
        .unwrap();
    lab.engine()
        .record_validity_assessment(RecordValidityAssessmentInput {
            specimen_id,
            policy_version_id: policies[0].id,
            category_code: "SYN-VALID".to_owned(),
            evidence_revision_ids: vec![evidence],
            supersedes_id: None,
            reason: None,
        })
        .await
}
