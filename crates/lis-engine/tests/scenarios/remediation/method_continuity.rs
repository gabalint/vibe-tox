//! WFREV-02: native engine and TCP import, with preserved issued bytes.
use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_lab::{Lab, Role, Sample};
use crate::m2_steps::{message, reapprove};
use crate::m3_fixture::{Bench, Staff, Variant};
use lis_analytical::{AttemptKind, ExecutionId, MaterialKind, QcOutcome, QcRole, QcScope, RunId};
use lis_catalog::{
    ComponentInput, MethodVersionContent, MethodVersionId, OrderableVersionContent,
    OrderableVersionId,
};
use lis_core::DecimalText;
use lis_engine::{
    AddPatientPositionInput, AddQcPositionInput, ApproveReviewedCatalogInput, AssessQcInput,
    CloseRunInput, CreateRunInput, DraftMethodVersion, DraftOrderableVersion, ExportReportPdfInput,
    GetMethodVersionInput, GetOrderInput, GetOrderableVersionInput, GetReportRevisionInput,
    GetRunInput, GetRunQcInput, GetSpecimenResultsInput, ListLotsInput,
    PrepareReportCandidateInput, SubmitTechnicalReviewInput,
};
use lis_reporting::{ExportPurpose, ReportRevisionId, RevisionSummary};

#[tokio::test]
async fn latest_attributable_review_controls_release_separation_of_duties() {
    let bench = Bench::ready(Variant::Panel).await;
    let revisions = vec![
        bench.revision("SYN-MOR").await,
        bench.revision("SYN-OXY").await,
    ];
    // T already approved these exact revisions. B now re-reviews the same
    // evidence at the fixture's unchanged instant (a repeat approval is refused,
    // so B rejects first), then attempts to release it as B.
    bench.sign_in(Staff::Racer).await;
    let review = reapprove(
        bench.lab().engine(),
        bench.sample().specimen_id,
        revisions.clone(),
    )
    .await;
    assert_eq!(review.reviewed_by, bench.user_id(Staff::Racer).to_string());
    let current = bench
        .lab()
        .engine()
        .get_specimen_results(GetSpecimenResultsInput {
            specimen_id: bench.sample().specimen_id,
        })
        .await
        .unwrap();
    let reviewed: Vec<_> = current
        .lines
        .iter()
        .filter(|line| {
            line.current
                .as_ref()
                .is_some_and(|r| revisions.contains(&r.revision_id))
        })
        .collect();
    assert_eq!(reviewed.len(), revisions.len());
    for line in reviewed {
        assert!(
            matches!(&line.review_status, lis_results::ReviewStatus::Current { review_id }
            if *review_id == review.review_id)
        );
    }
    let candidate = bench.prepare(None).await.unwrap();
    let refused = bench.release(&candidate).await.unwrap_err();
    assert_eq!(refused.code(), lis_core::ErrorCode::GateBlocked);
    assert!(
        serde_json::to_value(&refused).unwrap()["gateReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gate| gate["code"] == "self_release")
    );
    // A separate current releaser remains able to complete the workflow.
    bench.sign_in(Staff::Releaser).await;
    let separate = bench.prepare(None).await.unwrap();
    bench.release(&separate).await.unwrap();
}

#[tokio::test]
async fn accepted_v1_without_execution_completes_after_v2_and_issued_bytes_survive_repeats() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let issued = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let original = exported_copy(&bench, issued.revision_id).await;
    let lab = bench.desk().await;
    let pending = lab.add_sample(lab.orderables.panel.1).await;
    let executions: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM ana_executions WHERE container_id=?1",
            [pending.container_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        executions, 0,
        "accepted v1 material has no execution at activation"
    );
    let (v2, panel_v2) = activate_v2(lab).await;
    assert_version_and_scope(lab, &pending, v2, panel_v2).await;
    assert_eq!(exported_copy(&bench, issued.revision_id).await, original);
    let execution = complete_v1(bench.desk().await, &pending).await;
    let completed = release_v1(&bench, &pending).await;
    let completed_copy = exported_copy(&bench, completed.revision_id).await;
    repeat_v1(bench.desk().await, &pending, execution).await;
    assert_eq!(
        exported_copy(&bench, completed.revision_id).await,
        completed_copy
    );
    assert_eq!(exported_copy(&bench, issued.revision_id).await, original);
}

async fn assert_version_and_scope(
    lab: &Lab,
    pending: &Sample,
    v2: MethodVersionId,
    panel_v2: OrderableVersionId,
) {
    let current = lab
        .engine()
        .get_method_version(GetMethodVersionInput {
            method_version_id: v2,
        })
        .await
        .unwrap();
    assert_eq!(current.version, 2);
    assert!(current.lifecycle.effective_to.is_none());
    let old = lab
        .engine()
        .get_method_version(GetMethodVersionInput {
            method_version_id: lab.methods.drugs,
        })
        .await
        .unwrap();
    assert!(old.lifecycle.effective_to.is_some());
    assert_eq!(old.lifecycle.status, lis_catalog::VersionStatus::Approved);
    let fresh = lab.add_sample(panel_v2).await;
    let fresh_order = lab
        .engine()
        .get_order(GetOrderInput {
            order_id: fresh.order_id,
        })
        .await
        .unwrap();
    assert_eq!(fresh_order.items[0].orderable_version_id, panel_v2);
    let fresh_manifest = lab
        .engine()
        .get_orderable_version(GetOrderableVersionInput {
            orderable_version_id: panel_v2,
        })
        .await
        .unwrap();
    assert_eq!(fresh_manifest.components[0].method_version_id, v2);
    let retained = lab
        .engine()
        .get_order(GetOrderInput {
            order_id: pending.order_id,
        })
        .await
        .unwrap();
    assert_eq!(
        retained.items[0].orderable_version_id,
        lab.orderables.panel.1
    );
}

async fn complete_v1(lab: &Lab, pending: &Sample) -> ExecutionId {
    close_run(lab, lab.run).await;
    let run = new_run(lab, lab.methods.drugs).await;
    let position = lab
        .engine()
        .add_patient_position(AddPatientPositionInput {
            run_id: run,
            position_label: "P-CONTINUITY".into(),
            container_label: pending.label_code.clone(),
            attempt_kind: AttemptKind::Initial,
            related_execution_id: None,
            dilution_factor: None,
            prep_batch_id: None,
            reason: None,
        })
        .await
        .unwrap();
    let execution = position
        .positions
        .iter()
        .find(|p| p.label == "P-CONTINUITY")
        .unwrap()
        .execution
        .as_ref()
        .unwrap()
        .execution_id;
    import_and_accept_qc(lab, run, &pending.label_code).await;
    lab.sign_in(Role::Reviewer).await;
    let results = lab
        .engine()
        .get_specimen_results(GetSpecimenResultsInput {
            specimen_id: pending.specimen_id,
        })
        .await
        .unwrap();
    assert_eq!(results.lines.len(), 2);
    let revisions = results
        .lines
        .iter()
        .map(|line| {
            let current = line.current.as_ref().unwrap();
            assert_eq!(current.method_version_id, lab.methods.drugs);
            if line.analyte_code == "SYN-MOR" {
                assert_eq!(current.magnitude.as_ref().unwrap().as_str(), "152.40");
                assert_eq!(current.classification, "meets_cutoff");
            }
            current.revision_id
        })
        .collect();
    lab.engine()
        .submit_technical_review(SubmitTechnicalReviewInput {
            specimen_id: pending.specimen_id,
            decision: lis_results::ReviewDecision::Approved,
            result_revision_ids: revisions,
            reason: None,
        })
        .await
        .unwrap();
    close_run(lab, run).await;
    execution
}

async fn release_v1(bench: &Bench, pending: &Sample) -> RevisionSummary {
    let lab = bench.lab();
    bench.sign_in(Staff::Releaser).await;
    let candidate = lab
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            condition_review: None,
            specimen_id: pending.specimen_id,
            amendment_reason: None,
        })
        .await
        .unwrap();
    let json = serde_json::to_string(&candidate.content).unwrap();
    // The v2 batch metadata must not leak into this retained v1 report. The
    // referenced-method projection itself is independently source reviewed.
    assert!(!json.contains("SYN-EXTERNAL-APPROVAL-CONTINUITY"));
    assert_eq!(candidate.content.result_groups.len(), 1);
    assert!(
        candidate
            .content
            .result_groups
            .iter()
            .all(|group| group.method.contains("v1"))
    );
    bench.release(&candidate).await.unwrap()
}

// Exercise the native rerun entry point; domain tests cover all kinds and safety holds.
async fn repeat_v1(lab: &Lab, pending: &Sample, execution: ExecutionId) {
    let repeat_run = new_run(lab, lab.methods.drugs).await;
    let repeated = lab
        .engine()
        .add_patient_position(AddPatientPositionInput {
            run_id: repeat_run,
            position_label: "P-REPEAT".into(),
            container_label: pending.label_code.clone(),
            attempt_kind: AttemptKind::Rerun,
            related_execution_id: Some(execution),
            dilution_factor: None,
            prep_batch_id: None,
            reason: Some("Synthetic repeat under original approved version".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        repeated
            .positions
            .iter()
            .find(|p| p.label == "P-REPEAT")
            .unwrap()
            .execution
            .as_ref()
            .unwrap()
            .related_execution_id,
        Some(execution)
    );
    assert_eq!(
        lab.engine()
            .get_run(GetRunInput { run_id: repeat_run })
            .await
            .unwrap()
            .method_version_id,
        lab.methods.drugs
    );
}

async fn activate_v2(lab: &Lab) -> (MethodVersionId, OrderableVersionId) {
    lab.sign_in(Role::Admin).await;
    let e = lab.engine();
    let old = e
        .get_method_version(GetMethodVersionInput {
            method_version_id: lab.methods.drugs,
        })
        .await
        .unwrap();
    let next = e
        .draft_method_version(DraftMethodVersion {
            method_id: old.method_id,
            content: MethodVersionContent {
                report_notes: old.report_notes,
                specimen_type_id: old.specimen_type_id,
                certainty: old.certainty,
                output_scale: old.output_scale,
                threshold_rule: old.threshold_rule,
                threshold_rounding: old.threshold_rounding,
                analytes: old
                    .analytes
                    .into_iter()
                    .map(|a| lis_catalog::MethodAnalyte {
                        cutoff: Some(DecimalText::parse("500").unwrap()),
                        ..a.limits
                    })
                    .collect(),
            },
        })
        .await
        .unwrap()
        .id;
    let panel = draft_current_orderables(lab, next).await;
    let preview_id = e.generate_catalog_preview().await.unwrap();
    // This fixture user explicitly has Administrator + QualityManager. A second
    // administrator approves, preserving the configured separation of duties.
    lab.sign_in(Role::Quality).await;
    e.approve_reviewed_catalog(ApproveReviewedCatalogInput {
        preview_id,
        confirmed_blank_cutoffs: vec![],
        self_approval: false,
        external_approval_reference: Some("SYN-EXTERNAL-APPROVAL-CONTINUITY".into()),
        password: secret(NEW_PASSWORD).into(),
    })
    .await
    .unwrap();
    let authority: String = lab.t().conn().query_row(
        "SELECT authority FROM audit_events WHERE action='catalog.approve_reviewed_catalog' AND outcome='succeeded' ORDER BY seq DESC LIMIT 1", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(authority, "approve_configuration");
    (next, panel)
}

async fn draft_current_orderables(lab: &Lab, next: MethodVersionId) -> OrderableVersionId {
    let e = lab.engine();
    let mut panel = None;
    for (id, old_id) in [
        lab.orderables.panel,
        lab.orderables.confirmation,
        lab.orderables.with_validity,
    ] {
        let view = e
            .get_orderable_version(GetOrderableVersionInput {
                orderable_version_id: old_id,
            })
            .await
            .unwrap();
        let draft = e
            .draft_orderable_version(DraftOrderableVersion {
                orderable_id: id,
                content: OrderableVersionContent {
                    components: view
                        .components
                        .into_iter()
                        .map(|c| ComponentInput {
                            method_version_id: if c.method_version_id == lab.methods.drugs {
                                next
                            } else {
                                c.method_version_id
                            },
                            purpose: c.purpose,
                            analyte_ids: c.analytes.into_iter().map(|a| a.analyte_id).collect(),
                        })
                        .collect(),
                },
            })
            .await
            .unwrap();
        if old_id == lab.orderables.panel.1 {
            panel = Some(draft.id);
        }
    }
    panel.unwrap()
}

async fn close_run(lab: &Lab, run_id: RunId) {
    lab.sign_in(Role::Analyst).await;
    let run = lab.engine().get_run(GetRunInput { run_id }).await.unwrap();
    lab.engine()
        .close_run(CloseRunInput {
            run_id,
            expected_revision: run.revision,
            reason: None,
        })
        .await
        .unwrap();
}

async fn new_run(lab: &Lab, method_version_id: MethodVersionId) -> RunId {
    lab.sign_in(Role::Analyst).await;
    let controls = lab
        .engine()
        .list_lots(ListLotsInput { active_only: true })
        .await
        .unwrap()
        .into_iter()
        .filter(|lot| lot.material_kind == MaterialKind::Control)
        .collect::<Vec<_>>();
    let run_id = lab
        .engine()
        .create_run(CreateRunInput {
            instrument_id: lab.instrument,
            method_version_id,
            lot_ids: controls.iter().map(|l| l.lot_id).collect(),
        })
        .await
        .unwrap()
        .run_id;
    for (index, lot) in controls.iter().enumerate() {
        lab.engine()
            .add_qc_position(AddQcPositionInput {
                run_id,
                position_label: format!("Q{index}"),
                role: QcRole::Control,
                lot_id: Some(lot.lot_id),
                sample_label: None,
            })
            .await
            .unwrap();
    }
    run_id
}

async fn import_and_accept_qc(lab: &Lab, run_id: RunId, label: &str) {
    lab.import(&message(&[
        "P|1",
        "O|1|QC-LOW-SYN|SYN-CONTROL-LOW||R",
        "R|1|^^^SYN-MOR|37.6|ng/mL||N||F||SYN-TECH-01||20261012140000|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|38.1|ng/mL||N||F||SYN-TECH-01||20261012140000|SYNTH-LCMS-01",
        "P|2",
        &format!("O|1|{label}|SYN-CONTINUITY||R"),
        "R|1|^^^SYN-MOR|152.40|ng/mL||H||F||SYN-TECH-01||20261012140500|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|<5.0|ng/mL||N||F||SYN-TECH-01||20261012140500|SYNTH-LCMS-01",
        "P|3",
        "O|1|QC-HIGH-SYN|SYN-CONTROL-HIGH||R",
        "R|1|^^^SYN-MOR|187.9|ng/mL||N||F||SYN-TECH-01||20261012141000|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|191.2|ng/mL||N||F||SYN-TECH-01||20261012141000|SYNTH-LCMS-01",
    ]))
    .await;
    lab.sign_in(Role::Quality).await;
    let qc = lab
        .engine()
        .get_run_qc(GetRunQcInput { run_id })
        .await
        .unwrap();
    for entry in qc.analytes {
        let evidence = entry
            .observations
            .iter()
            .filter(|o| !o.superseded)
            .map(|o| o.observation_id)
            .collect::<Vec<_>>();
        assert_eq!(evidence.len(), 2);
        lab.engine()
            .assess_qc(AssessQcInput {
                scope: QcScope::RunAnalyte,
                run_id: Some(run_id),
                analyte_id: Some(entry.analyte_id),
                prep_batch_id: None,
                outcome: QcOutcome::Accepted,
                evidence_observation_ids: evidence,
                supersedes_assessment_id: None,
                reason: None,
            })
            .await
            .unwrap();
    }
}

async fn exported_copy(bench: &Bench, revision_id: ReportRevisionId) -> (String, Vec<u8>) {
    bench.sign_in(Staff::Releaser).await;
    let e = bench.lab().engine();
    let revision = e
        .get_report_revision(GetReportRevisionInput { revision_id })
        .await
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let exported = e
        .export_report_pdf(
            ExportReportPdfInput {
                revision_id,
                purpose: ExportPurpose::Records,
                obligation_id: None,
            },
            Some(directory.path().to_owned()),
        )
        .await
        .unwrap();
    (
        serde_json::to_string(&revision.content).unwrap(),
        std::fs::read(directory.path().join(exported.file_name)).unwrap(),
    )
}
