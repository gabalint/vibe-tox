//! Result-review calls for the registry-wide authorization table
//! (REQ-RES-005, NFR-08). Plausible synthetic IDs ensure authorization is
//! checked before any record lookup or gate evaluation.

use lis_analytical::{QcConflictState, RevisionIndicator, SampleIdField, SourceTestCodeInput};
use lis_core::{CommandError, ErrorCode, UserId};
use lis_engine::{
    ApproveSourceInput, CloseDeviationInput, CloseRunInput, DeviationTarget, Engine,
    FailExecutionInput, GetImportAttemptInput, GetRunInput, GetRunQcInput, GetSpecimenResultsInput,
    ImportAttemptId, ListImportAttemptsInput, ListReviewQueueInput, ListValidityPoliciesInput,
    MarkContainerDepletedInput, OpenDeviationInput, PlaceContainerHoldInput, QuarantineItemId,
    QuarantineResolution, RecordResultDispositionInput, RecordValidityAssessmentInput,
    RegisterSourceInput, ResolveQuarantineItemInput, ResultId, ResultOutcome, ResultRevisionId,
    RetryImportInput, ReviewQueueFilter, SelectResultEvidenceInput, SubmitTechnicalReviewInput,
};
use lis_results::{
    ImportOutcome, QuarantineReason, ReleaseReviewStatus, ResultCause, ReviewStatus,
};
use lis_specimens::ContainerHoldKind;

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{CORRECTION, JOURNEY, gate_codes, line, message};

pub(crate) async fn results_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    let specimen_id = lis_specimens::SpecimenId::generate();
    let result_id = ResultId::generate();
    let revision_id = ResultRevisionId::generate();
    Some(match name {
        "list_review_queue" => engine
            .list_review_queue(ListReviewQueueInput {
                after: None,
                filter: ReviewQueueFilter::Ready,
                limit: 10,
            })
            .await
            .map(drop),
        "get_specimen_results" => engine
            .get_specimen_results(GetSpecimenResultsInput { specimen_id })
            .await
            .map(drop),
        "select_result_evidence" => engine
            .select_result_evidence(SelectResultEvidenceInput {
                result_id,
                expected_revision: 1,
                measurement_ids: vec![lis_engine::MeasurementId::generate()],
                reason: reason(),
            })
            .await
            .map(drop),
        "record_result_disposition" => engine
            .record_result_disposition(RecordResultDispositionInput {
                result_id: Some(result_id),
                expected_revision: 1,
                component_id: None,
                analyte_id: None,
                specimen_id: None,
                outcome: ResultOutcome::Missing,
                reason: reason(),
            })
            .await
            .map(drop),
        "submit_technical_review" => engine
            .submit_technical_review(SubmitTechnicalReviewInput {
                specimen_id,
                decision: lis_results::ReviewDecision::Rejected,
                result_revision_ids: vec![revision_id],
                reason: Some(reason()),
            })
            .await
            .map(drop),
        "list_validity_policies" => engine
            .list_validity_policies(ListValidityPoliciesInput {
                specimen_type_code: "SYN".to_owned(),
            })
            .await
            .map(drop),
        "record_validity_assessment" => engine
            .record_validity_assessment(RecordValidityAssessmentInput {
                specimen_id,
                policy_version_id: lis_catalog::ValidityPolicyVersionId::generate(),
                category_code: "synthetic".to_owned(),
                evidence_revision_ids: vec![revision_id],
                supersedes_id: None,
                reason: None,
            })
            .await
            .map(drop),
        _ => return import_and_reflex_call(engine, name, revision_id).await,
    })
}

async fn import_and_reflex_call(
    engine: &Engine,
    name: &str,
    revision_id: ResultRevisionId,
) -> Option<Result<(), CommandError>> {
    Some(match name {
        "list_import_attempts" => engine
            .list_import_attempts(ListImportAttemptsInput {
                after_import_attempt_id: None,
                instrument_id: None,
                limit: 10,
                before: None,
            })
            .await
            .map(drop),
        "get_import_attempt" => engine
            .get_import_attempt(GetImportAttemptInput {
                import_attempt_id: lis_engine::ImportAttemptId::generate(),
            })
            .await
            .map(drop),
        "resolve_quarantine_item" => engine
            .resolve_quarantine_item(ResolveQuarantineItemInput {
                item_id: QuarantineItemId::generate(),
                expected_revision: 1,
                resolution: QuarantineResolution::Reject,
                reason: reason(),
            })
            .await
            .map(drop),
        "retry_import" => engine
            .retry_import(RetryImportInput {
                receipt_id: lis_interfaces::ReceiptId::generate(),
                reason: reason(),
            })
            .await
            .map(drop),
        "list_reflex_triggers" => engine
            .list_reflex_triggers(lis_engine::ListReflexTriggersInput {
                specimen_id: lis_specimens::SpecimenId::generate(),
            })
            .await
            .map(drop),
        "record_reflex_decision" => engine
            .record_reflex_decision(lis_engine::RecordReflexDecisionInput {
                parent_item_id: lis_engine::OrderItemId::generate(),
                orderable_id: lis_catalog::OrderableId::generate(),
                result_revision_id: revision_id,
                decision: lis_engine::ReflexDecision::ConditionNotMet,
                sequence_role: None,
                reason: reason(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn reason() -> String {
    "Synthetic authorization test".to_owned()
}

#[tokio::test]
async fn changed_qc_row_is_owned_only_by_the_analytical_conflict_queue() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let attempt = lab
        .import(&message(&[
            "P|1",
            "O|1|QC-LOW-SYN|SYN-POS-01||R",
            "R|1|^^^SYN-MOR|37.9|ng/mL||N||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
        ]))
        .await;
    assert_eq!(attempt.outcome, ImportOutcome::Processed);
    assert_eq!((attempt.rows_qc, attempt.rows_quarantined), (1, 0));

    let imported = lab
        .engine()
        .get_import_attempt(GetImportAttemptInput {
            import_attempt_id: attempt.import_attempt_id,
        })
        .await
        .unwrap();
    assert!(imported.quarantine.is_empty());
    lab.sign_in(Role::Quality).await;
    let qc = lab
        .engine()
        .get_run_qc(GetRunQcInput { run_id: lab.run })
        .await
        .unwrap();
    assert_eq!(qc.conflicts.len(), 1);
    assert_eq!(qc.conflicts[0].state, QcConflictState::Open);

    let mor = line(&lab.results(0).await, "SYN-MOR")
        .current
        .as_ref()
        .unwrap()
        .revision_id;
    let refused = lab.approve(0, vec![mor]).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert!(gate_codes(&refused).contains(&"qc_conflict".to_owned()));
    lab.engine().shutdown().await;
}

#[tokio::test]
async fn failed_execution_quarantines_matched_patient_rows_without_measurements() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.sign_in(Role::Analyst).await;
    let run = lab
        .engine()
        .get_run(GetRunInput { run_id: lab.run })
        .await
        .unwrap();
    let execution = run
        .positions
        .iter()
        .find(|position| position.sample_label == lab.samples[0].label_code)
        .and_then(|position| position.execution.as_ref())
        .unwrap();
    lab.engine()
        .fail_execution(FailExecutionInput {
            execution_id: execution.execution_id,
            expected_revision: execution.revision,
            reason: "Synthetic instrument interruption".to_owned(),
        })
        .await
        .unwrap();

    let label = &lab.samples[0].label_code;
    let attempt = lab
        .import(&message(&[
            "P|1",
            &format!("O|1|{label}|SYN-POS-02||R"),
            "R|1|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        ]))
        .await;
    assert_eq!((attempt.rows_new, attempt.rows_quarantined), (0, 1));
    let detail = lab
        .engine()
        .get_import_attempt(GetImportAttemptInput {
            import_attempt_id: attempt.import_attempt_id,
        })
        .await
        .unwrap();
    assert_eq!(detail.quarantine.len(), 1);
    assert_eq!(
        detail.quarantine[0].reason,
        QuarantineReason::ExecutionFailed
    );
    let measurements: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM res_measurements measurement \
             JOIN ana_v_positions position ON position.position_id = measurement.position_id \
             WHERE position.container_id = ?1",
            [lab.samples[0].container_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(measurements, 0);
    lab.engine().shutdown().await;
}

#[tokio::test]
async fn review_uses_release_gate_after_material_is_depleted_and_still_honors_holds() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let revision: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT revision FROM spc_containers WHERE id = ?1",
            [lab.samples[0].container_id],
            |row| row.get(0),
        )
        .unwrap();
    lab.sign_in(Role::Accessioner).await;
    lab.engine()
        .mark_container_depleted(MarkContainerDepletedInput {
            container_id: lab.samples[0].container_id,
            expected_revision: revision,
            reason: "Synthetic testing used the remaining volume".to_owned(),
        })
        .await
        .unwrap();
    lab.approve_all(0).await.unwrap();

    lab.sign_in(Role::Quality).await;
    lab.engine()
        .place_hold(PlaceContainerHoldInput {
            container_id: lab.samples[1].container_id,
            kind: ContainerHoldKind::Quality,
            reason: "Synthetic container integrity review".to_owned(),
        })
        .await
        .unwrap();
    let held = lab.approve_all(1).await.unwrap_err();
    assert_eq!(held.code(), ErrorCode::GateBlocked);
    assert!(gate_codes(&held).contains(&"container_on_hold".to_owned()));
    lab.engine().shutdown().await;
}

#[tokio::test]
async fn deviation_opened_and_closed_after_review_keeps_the_review_stale() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let review = lab.approve_all(0).await.unwrap();
    let stored: String = lab
        .t()
        .conn()
        .query_row(
            "SELECT ref_id FROM res_review_dependencies \
             WHERE review_id = ?1 AND kind = 'deviation_watermark'",
            [review.review_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, "0");

    lab.sign_in(Role::Quality).await;
    let deviation = lab
        .engine()
        .open_deviation(OpenDeviationInput {
            scope: DeviationTarget::Run(lab.run),
            analyte_id: Some(lab.analytes.mor),
            description: "Synthetic drift investigation".to_owned(),
            reason: "Synthetic post-review observation".to_owned(),
        })
        .await
        .unwrap();
    lab.engine()
        .close_deviation(CloseDeviationInput {
            deviation_id: deviation.deviation_id,
            expected_revision: deviation.revision,
            disposition: "Synthetic review impact assessed".to_owned(),
            reason: "Synthetic investigation complete".to_owned(),
        })
        .await
        .unwrap();
    // The deviation opened after the review is a durable cause: closing it does not make
    // the review current again (M2b review finding 7; M3 impact keys name their cause).
    let results = lab.results(0).await;
    assert!(matches!(
        &line(&results, "SYN-MOR").review_status,
        ReviewStatus::Stale { reasons }
            if reasons.iter().any(|reason| reason.code == "deviation_open")
    ));
    lab.engine().shutdown().await;
}

#[tokio::test]
async fn review_without_a_deviation_watermark_is_conservatively_stale() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let review = lab.approve_all(0).await.unwrap();
    let revision = line(&lab.results(0).await, "SYN-MOR")
        .current
        .as_ref()
        .unwrap()
        .revision_id;
    let legacy_review = lis_results::TechnicalReviewId::generate();
    let conn = lab.t().conn();
    conn.execute(
        "INSERT INTO res_technical_reviews \
         (id, specimen_id, decision, reason, reviewed_by, reviewed_at, sequence) \
         SELECT ?1, ?2, 'approved', NULL, ?3, '2026-10-14T12:00:00.000000000Z', \
                max(sequence) + 1 FROM res_technical_reviews",
        rusqlite::params![
            legacy_review,
            lab.samples[0].specimen_id,
            review.reviewed_by
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO res_review_items (review_id, result_revision_id) VALUES (?1, ?2)",
        rusqlite::params![legacy_review, revision],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO res_review_dependencies (review_id, kind, ref_id) \
         SELECT ?1, kind, ref_id FROM res_review_dependencies \
         WHERE review_id = ?2 AND kind <> 'deviation_watermark'",
        rusqlite::params![legacy_review, review.review_id],
    )
    .unwrap();

    let results = lab.results(0).await;
    // This directly inserted legacy review has no unique clinical audit evidence.
    assert!(matches!(
        &line(&results, "SYN-MOR").review_status,
        ReviewStatus::Stale { reasons }
            if reasons.iter().any(|reason| reason.code == "review_chronology_unresolved")
    ));
    lab.engine().shutdown().await;
}

#[tokio::test]
async fn correction_is_found_by_the_stale_queue_and_leaves_the_reviewed_lines_view() {
    let lab = Lab::start(RevisionIndicator::ResultStatusCorrection).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.approve_all(0).await.unwrap();
    lab.import(CORRECTION).await;

    let queue = lab
        .engine()
        .list_review_queue(ListReviewQueueInput {
            after: None,
            filter: ReviewQueueFilter::Stale,
            limit: 100,
        })
        .await
        .unwrap();
    assert!(
        queue
            .items
            .iter()
            .any(|entry| { entry.specimen_id == lab.samples[0].specimen_id && entry.stale >= 1 })
    );
    let reviewed_mor: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM res_v_reviewed_lines \
             WHERE specimen_id = ?1 AND analyte_id = ?2",
            rusqlite::params![lab.samples[0].specimen_id, lab.analytes.mor],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reviewed_mor, 0);
    lab.engine().shutdown().await;
}

#[tokio::test]
async fn m3_release_queries_return_current_review_and_out_of_scope_lineage() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let review = lab.approve_all(0).await.unwrap();
    let lines =
        lis_results::specimen_release_lines(&lab.t().conn(), lab.samples[0].specimen_id).unwrap();
    assert_eq!(lines.len(), 2);
    let mor = lines
        .iter()
        .find(|release| release.analyte_id == lab.analytes.mor)
        .unwrap();
    assert_eq!(
        mor.current
            .as_ref()
            .unwrap()
            .magnitude
            .as_ref()
            .unwrap()
            .as_str(),
        "152.40"
    );
    assert!(matches!(
        mor.review,
        ReleaseReviewStatus::Current { review_id, .. } if review_id == review.review_id
    ));
    assert!(!mor.open_quarantine);
    assert!(
        lis_results::current_validity(&lab.t().conn(), lab.samples[0].specimen_id)
            .unwrap()
            .is_none()
    );
    let mor_revision = mor.current.as_ref().unwrap().revision_id;
    reject_and_assert_release_line(&lab, mor_revision).await;

    let confirmation = lab.add_sample(lab.orderables.confirmation.1).await;
    lab.place(&confirmation).await;
    let label = &confirmation.label_code;
    let attempt = lab
        .import(&message(&[
            "P|1",
            &format!("O|1|{label}|SYN-POS-02||R"),
            "R|1|^^^SYN-OXY|70.0|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        ]))
        .await;
    assert_eq!(attempt.rows_quarantined, 1);
    let detections =
        lis_results::out_of_scope_detections(&lab.t().conn(), confirmation.specimen_id).unwrap();
    assert_eq!(detections.len(), 1);
    assert_eq!(detections[0].source_test_code, "SYN-OXY");
    assert_eq!(detections[0].state, lis_results::QuarantineState::Open);
    lab.engine().shutdown().await;
}

async fn reject_and_assert_release_line(lab: &Lab, revision_id: ResultRevisionId) {
    lab.engine()
        .submit_technical_review(SubmitTechnicalReviewInput {
            specimen_id: lab.samples[0].specimen_id,
            decision: lis_results::ReviewDecision::Rejected,
            result_revision_ids: vec![revision_id],
            reason: Some("Synthetic technical rejection".to_owned()),
        })
        .await
        .unwrap();
    let rejected =
        lis_results::specimen_release_lines(&lab.t().conn(), lab.samples[0].specimen_id).unwrap();
    let mor = rejected
        .iter()
        .find(|release| release.analyte_id == lab.analytes.mor)
        .unwrap();
    assert!(matches!(
        &mor.review,
        ReleaseReviewStatus::Stale { reasons }
            if reasons.iter().any(|reason| reason.code == "review_rejected")
    ));
    let reviewed: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM res_v_reviewed_lines \
             WHERE specimen_id = ?1 AND analyte_id = ?2",
            rusqlite::params![lab.samples[0].specimen_id, lab.analytes.mor],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reviewed, 0);
}

// REQ-RES-005, NFR-01: commit order governs rejections and subsequent
// approvals, even when the host clock repeats or moves backwards.
#[tokio::test]
async fn review_decisions_follow_commit_order_after_clock_changes() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let first = lab.approve_all(0).await.unwrap();
    let revision = line(&lab.results(0).await, "SYN-MOR")
        .current
        .as_ref()
        .unwrap()
        .revision_id;

    lab.t().advance(60);
    reject_and_assert_release_line(&lab, revision).await;
    lab.t().clock.set(first.reviewed_at);
    approve_and_assert_current_review(&lab, revision).await;

    // A second decision at the same timestamp is still newer.
    reject_and_assert_release_line(&lab, revision).await;
    approve_and_assert_current_review(&lab, revision).await;

    // A rejection after a clock rollback must also leave the release queue.
    lab.t().advance(60);
    approve_and_assert_current_review(&lab, revision).await;
    lab.t().clock.set(first.reviewed_at);
    reject_and_assert_release_line(&lab, revision).await;
    approve_and_assert_current_review(&lab, revision).await;
    lab.engine().shutdown().await;
}

async fn approve_and_assert_current_review(lab: &Lab, revision: ResultRevisionId) {
    let approved = lab.approve(0, vec![revision]).await.unwrap();
    let released =
        lis_results::specimen_release_lines(&lab.t().conn(), lab.samples[0].specimen_id).unwrap();
    let mor = released
        .iter()
        .find(|line| line.analyte_id == lab.analytes.mor)
        .unwrap();
    assert!(matches!(
        mor.review,
        ReleaseReviewStatus::Current { review_id, .. } if review_id == approved.review_id
    ));
    let discovered: lis_results::TechnicalReviewId = lab
        .t()
        .conn()
        .query_row(
            "SELECT review_id FROM res_v_reviewed_lines \
             WHERE specimen_id = ?1 AND analyte_id = ?2",
            rusqlite::params![lab.samples[0].specimen_id, lab.analytes.mor],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(discovered, approved.review_id);
}

#[tokio::test]
async fn one_receipt_plans_repeats_sequentially_and_quarantine_keeps_its_context() {
    let lab = Lab::start(RevisionIndicator::ResultStatusCorrection).await;
    let attempt_id = import_repeated_rows(&lab).await;
    // A mixed new/corrected batch has one processing revision for the run,
    // not additional empty revisions and reprocessing events for every row.
    let processing_revisions: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM res_processing_revisions WHERE import_attempt_id = ?1",
            [attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(processing_revisions, 1);
    let reprocess_events = lab
        .t()
        .events("clinical")
        .into_iter()
        .filter(|(action, _)| action == "result.reprocess")
        .count();
    assert_eq!(reprocess_events, 1);
    let [rotation_item, intervening_item, closed_run_item] = conflict_items(&lab, attempt_id).await;
    assert_registration_and_intervening_conflicts(&lab, rotation_item, intervening_item).await;
    assert_closed_run_missing_and_access(&lab, closed_run_item).await;
    lab.engine().shutdown().await;
}

async fn import_repeated_rows(lab: &Lab) -> ImportAttemptId {
    let artifact = message(&[
        "P|1",
        "O|1|C00000001|SYN-POS-02||R",
        "R|1|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "R|2|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "P|2",
        "O|1|C00000002|SYN-POS-03||R",
        "R|1|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "R|2|^^^SYN-MOR|200|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "P|3",
        "O|1|C00000003|SYN-POS-04||R",
        "R|1|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "R|2|^^^SYN-MOR|90|ng/mL||N||C||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "P|4",
        "O|1|C00000004|SYN-POS-06||R",
        "R|1|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "R|2|^^^SYN-MOR|200|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "P|5",
        "O|1|C00000005|SYN-POS-07||R",
        "R|1|^^^SYN-MOR|100|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
        "R|2|^^^SYN-MOR|200|ng/mL||N||F||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
    ]);
    let attempt = lab.import(&artifact).await;
    assert_eq!(attempt.outcome, ImportOutcome::ProcessedWithQuarantine);
    assert_eq!(
        (
            attempt.rows_total,
            attempt.rows_new,
            attempt.rows_duplicate,
            attempt.rows_reprocessed,
            attempt.rows_quarantined,
        ),
        (10, 5, 1, 1, 3)
    );
    assert_eq!(line(&lab.results(0).await, "SYN-MOR").history.len(), 1);
    let corrected = lab.results(2).await;
    let corrected = line(&corrected, "SYN-MOR");
    assert_eq!(corrected.history.len(), 2);
    assert_eq!(
        corrected.current.as_ref().unwrap().cause,
        ResultCause::Reprocessed
    );
    attempt.import_attempt_id
}

async fn conflict_items(lab: &Lab, import_attempt_id: ImportAttemptId) -> [QuarantineItemId; 3] {
    lab.sign_in(Role::Reviewer).await;
    let detail = lab
        .engine()
        .get_import_attempt(GetImportAttemptInput { import_attempt_id })
        .await
        .unwrap();
    let conflict_for = |label: &str| {
        detail
            .quarantine
            .iter()
            .find(|item| item.sample_label.as_deref() == Some(label))
            .unwrap()
            .item_id
    };
    [
        conflict_for("C00000002"),
        conflict_for("C00000004"),
        conflict_for("C00000005"),
    ]
}

async fn assert_registration_and_intervening_conflicts(
    lab: &Lab,
    rotation_item: QuarantineItemId,
    intervening_item: QuarantineItemId,
) {
    let correction = message(&[
        "P|1",
        "O|1|C00000004|SYN-POS-06||R",
        "R|1|^^^SYN-MOR|80|ng/mL||N||C||SYN-TECH-01||20261012120000|SYNTH-LCMS-01",
    ]);
    let correction_attempt = lab.import(&correction).await;
    assert_eq!(correction_attempt.rows_reprocessed, 1);

    rotate_registration(lab).await;
    lab.sign_in(Role::Reviewer).await;
    lab.engine()
        .resolve_quarantine_item(ResolveQuarantineItemInput {
            item_id: rotation_item,
            expected_revision: 1,
            resolution: QuarantineResolution::AcceptAsReprocessing,
            reason: "Synthetic reviewed conflict".to_owned(),
        })
        .await
        .unwrap();
    let rotated = lab.results(1).await;
    assert_eq!(line(&rotated, "SYN-MOR").history.len(), 2);
    assert!(
        rotated
            .lines
            .iter()
            .all(|entry| entry.analyte_code != "SYN-OXY")
    );
    let intervening = lab
        .engine()
        .resolve_quarantine_item(ResolveQuarantineItemInput {
            item_id: intervening_item,
            expected_revision: 1,
            resolution: QuarantineResolution::AcceptAsReprocessing,
            reason: "Synthetic stale conflict".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(intervening.code(), ErrorCode::Conflict);
}

async fn assert_closed_run_missing_and_access(lab: &Lab, closed_run_item: QuarantineItemId) {
    let measurements_before = measurement_count(lab, 4);
    lab.sign_in(Role::Analyst).await;
    lab.engine()
        .close_run(CloseRunInput {
            run_id: lab.run,
            expected_revision: 1,
            reason: Some("Synthetic completed run".to_owned()),
        })
        .await
        .unwrap();
    lab.sign_in(Role::Reviewer).await;
    let closed = lab
        .engine()
        .resolve_quarantine_item(ResolveQuarantineItemInput {
            item_id: closed_run_item,
            expected_revision: 1,
            resolution: QuarantineResolution::AcceptAsReprocessing,
            reason: "Synthetic closed-run conflict".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(closed.code(), ErrorCode::GateBlocked);
    assert!(gate_codes(&closed).contains(&"run_closed".to_owned()));
    let state: String = lab
        .t()
        .conn()
        .query_row(
            "SELECT state FROM res_quarantine_items WHERE id = ?1",
            [closed_run_item],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "open");
    assert_eq!(measurement_count(lab, 4), measurements_before);

    let pending = lab
        .results(4)
        .await
        .pending_analytes
        .into_iter()
        .find(|item| item.analyte_code == "SYN-OXY")
        .unwrap();
    let missing = lab
        .engine()
        .record_result_disposition(RecordResultDispositionInput {
            result_id: None,
            expected_revision: 0,
            component_id: Some(pending.component_id),
            analyte_id: Some(pending.analyte_id),
            specimen_id: Some(lab.samples[4].specimen_id),
            outcome: ResultOutcome::Missing,
            reason: "Synthetic analyzer omission".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(missing.outcome, ResultOutcome::Missing);
    assert!(missing.evidence.is_empty());
    lab.engine()
        .list_review_queue(ListReviewQueueInput {
            after: None,
            filter: ReviewQueueFilter::Blocked,
            limit: 100,
        })
        .await
        .unwrap();
    assert_access_actions(
        lab,
        &[
            "result.record_disposition",
            "result.list_review_queue",
            "result.resolve_quarantine",
        ],
    );
}

fn measurement_count(lab: &Lab, sample_index: usize) -> i64 {
    lab.t()
        .conn()
        .query_row(
            "SELECT count(*) FROM res_measurements measurement \
             JOIN ana_v_positions position ON position.position_id = measurement.position_id \
             WHERE position.container_id = ?1",
            [lab.samples[sample_index].container_id],
            |row| row.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn superseding_validity_evidence_blocks_review_and_stales_approval() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let (specimen_id, drug, validity) = validity_lines(&lab).await;
    lab.sign_in(Role::Reviewer).await;
    let policy = lab
        .engine()
        .list_validity_policies(ListValidityPoliciesInput {
            specimen_type_code: "SYN-UR".to_owned(),
        })
        .await
        .unwrap()[0]
        .id;
    let validity_assessment = lab
        .engine()
        .record_validity_assessment(RecordValidityAssessmentInput {
            specimen_id,
            policy_version_id: policy,
            category_code: "SYN-VALID".to_owned(),
            evidence_revision_ids: vec![validity.1],
            supersedes_id: None,
            reason: None,
        })
        .await
        .unwrap();
    assert_current_validity(&lab, specimen_id, &validity_assessment);
    let approved = lab
        .engine()
        .submit_technical_review(SubmitTechnicalReviewInput {
            specimen_id,
            decision: lis_results::ReviewDecision::Approved,
            result_revision_ids: vec![drug.1],
            reason: None,
        })
        .await
        .unwrap();
    lab.engine()
        .record_result_disposition(RecordResultDispositionInput {
            result_id: Some(validity.0),
            expected_revision: 2,
            component_id: None,
            analyte_id: None,
            specimen_id: None,
            outcome: ResultOutcome::Invalid,
            reason: "Synthetic validity correction".to_owned(),
        })
        .await
        .unwrap();
    let detail = lab
        .engine()
        .get_specimen_results(GetSpecimenResultsInput { specimen_id })
        .await
        .unwrap();
    assert!(
        lis_results::current_validity(&lab.t().conn(), specimen_id)
            .unwrap()
            .is_none()
    );
    let drug_line = detail
        .lines
        .iter()
        .find(|entry| entry.result_id == drug.0)
        .unwrap();
    assert!(
        drug_line
            .gates
            .iter()
            .any(|gate| gate.code == "validity_assessment_missing")
    );
    assert!(matches!(
        &drug_line.review_status,
        ReviewStatus::Stale { reasons }
            if reasons.iter().any(|reason| reason.code == "validity_assessment_stale")
    ));
    assert_ne!(
        drug_line.review_status,
        ReviewStatus::Current {
            review_id: approved.review_id
        }
    );
    assert_access_actions(&lab, &["result.assess_validity", "result.submit_review"]);
    lab.engine().shutdown().await;
}

fn assert_current_validity(
    lab: &Lab,
    specimen_id: lis_specimens::SpecimenId,
    expected: &lis_results::ValidityAssessmentView,
) {
    let current = lis_results::current_validity(&lab.t().conn(), specimen_id)
        .unwrap()
        .unwrap();
    assert_eq!(current.assessment_id, expected.assessment_id);
    assert_eq!(current.policy_version_id, expected.policy_version_id);
    assert_eq!(current.category_code, expected.category_code);
}

fn assert_access_actions(lab: &Lab, expected: &[&str]) {
    let access = lab.t().events("access");
    for expected in expected {
        assert!(access.iter().any(|(action, _)| action == expected));
    }
}

async fn validity_lines(
    lab: &Lab,
) -> (
    lis_specimens::SpecimenId,
    (ResultId, ResultRevisionId),
    (ResultId, ResultRevisionId),
) {
    let sample = lab.add_sample(lab.orderables.with_validity.1).await;
    let (analytical_component, validity_component): (
        lis_orders::OrderComponentId,
        lis_orders::OrderComponentId,
    ) = lab
        .t()
        .conn()
        .query_row(
            "SELECT \
               max(CASE WHEN purpose = 'analytical' THEN component_id END), \
               max(CASE WHEN purpose = 'validity' THEN component_id END) \
             FROM ord_v_components WHERE item_id = ?1",
            [sample.item_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let drug = insert_disposition_line(
        lab,
        analytical_component,
        lab.analytes.mor,
        sample.specimen_id,
        lab.methods.drugs,
    );
    let validity = insert_disposition_line(
        lab,
        validity_component,
        lab.analytes.cre,
        sample.specimen_id,
        lab.methods.validity,
    );
    (sample.specimen_id, drug, validity)
}

async fn rotate_registration(lab: &Lab) {
    lab.sign_in(Role::Admin).await;
    let draft = lab
        .engine()
        .register_source(RegisterSourceInput {
            au_profile: None,
            instrument_id: lab.instrument,
            sender_name: "SYNTH-LCMS".to_owned(),
            sender_version: "1.0".to_owned(),
            sample_id_field: SampleIdField::InstrumentSpecimenId,
            revision_indicator: RevisionIndicator::ResultStatusCorrection,
            reports_final_concentration: false,
            test_codes: vec![SourceTestCodeInput {
                source_test_code: "SYN-MOR".to_owned(),
                analyte_id: lab.analytes.oxy,
            }],
        })
        .await
        .unwrap();
    lab.sign_in(Role::Quality).await;
    lab.engine()
        .approve_source(ApproveSourceInput {
            registration_id: draft.registration_id,
            expected_revision: draft.revision,
            self_approval: false,
        })
        .await
        .unwrap();
}

fn insert_disposition_line(
    lab: &Lab,
    component_id: lis_orders::OrderComponentId,
    analyte_id: lis_catalog::AnalyteId,
    specimen_id: lis_specimens::SpecimenId,
    method_version_id: lis_catalog::MethodVersionId,
) -> (ResultId, ResultRevisionId) {
    let result_id = ResultId::generate();
    let revision_id = ResultRevisionId::generate();
    let conn = lab.t().conn();
    conn.execute(
        "INSERT INTO res_results \
         (id, order_component_id, analyte_id, purpose, current_revision_id, revision) \
         SELECT ?1, ?2, ?3, purpose, NULL, 1 FROM ord_v_components WHERE component_id = ?2",
        rusqlite::params![result_id, component_id, analyte_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO res_result_revisions (id, result_id, revision_no, cause, outcome, \
         classification, method_version_id, method_certainty, output_scale, sequence_role, \
         specimen_id, reason, created_by, created_at) \
         VALUES (?1, ?2, 1, 'disposition', 'missing', 'not_applicable', ?3, 'definitive', \
                 'quantitative', 'first_line', ?4, 'Synthetic missing result', 'synthetic-test', \
                 '2026-10-13T12:00:00.000000000Z')",
        rusqlite::params![revision_id, result_id, method_version_id, specimen_id],
    )
    .unwrap();
    conn.execute(
        "UPDATE res_results SET current_revision_id = ?2, revision = 2 WHERE id = ?1",
        rusqlite::params![result_id, revision_id],
    )
    .unwrap();
    (result_id, revision_id)
}
