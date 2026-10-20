//! RPT-01: native receipt-to-report disclosure and immutable issued artifacts.
use super::*;
use lis_reporting::{ConditionReportDecision, ConditionReportReview, ConditionReviewSource};
use lis_specimens::{DiscrepancyInput, ReceiptDiscrepancyCode};

async fn condition_desk() -> Desk {
    let mut desk = Desk::start().await;
    let sample = desk
        .lab
        .add_sample_with_receipt(
            desk.lab.orderables.panel.1,
            vec![
                DiscrepancyInput {
                    code: ReceiptDiscrepancyCode::Leaking,
                    note: Some("Synthetic seal leakage observed".to_owned()),
                },
                DiscrepancyInput {
                    code: ReceiptDiscrepancyCode::Other,
                    note: Some("Synthetic internal routing evidence".to_owned()),
                },
            ],
        )
        .await;
    desk.lab.place(&sample).await;
    desk.lab.samples.push(sample.clone());
    let order = format!("O|1|{}|SYN-COND||R", sample.label_code);
    let message = crate::m2_steps::message(&[
        "P|1",
        &order,
        "R|1|^^^SYN-MOR|152.40|ng/mL||H||F||SYN-TECH-01||20261012091500|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|<5.0|ng/mL||N||F||SYN-TECH-01||20261012091500|SYNTH-LCMS-01",
    ]);
    desk.lab.import(&message).await;
    desk.lab.approve_all(5).await.unwrap();
    desk.lab.samples.swap(0, 5);
    desk
}

fn choices(source: &ConditionReviewSource, include: bool) -> ConditionReportReview {
    ConditionReportReview {
        source_sha256: source.source_sha256.clone(),
        decisions: source
            .conditions
            .iter()
            .map(|condition| ConditionReportDecision {
                decision_id: condition.decision_id,
                code: condition.code,
                include_in_report: include && condition.code == ReceiptDiscrepancyCode::Leaking,
            })
            .collect(),
        reason: "Synthetic reportability assessment; source remains internal".to_owned(),
    }
}

async fn prepare_with(
    desk: &Desk,
    review: Option<ConditionReportReview>,
    reason: Option<&str>,
) -> Result<CandidateView, CommandError> {
    desk.as_releaser().await;
    let result = desk
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            specimen_id: desk.lab.samples[0].specimen_id,
            condition_review: review,
            amendment_reason: reason.map(str::to_owned),
        })
        .await;
    resync(&desk.lab).await;
    result
}

async fn exported(desk: &Desk, revision_id: ReportRevisionId) -> Vec<u8> {
    desk.as_releaser().await;
    let directory = tempfile::tempdir().unwrap();
    let file = desk
        .engine()
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
    resync(&desk.lab).await;
    std::fs::read(directory.path().join(file.file_name)).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_condition_review_preserves_raw_source_and_issued_bytes() {
    let desk = condition_desk().await;
    desk.as_releaser().await;
    let workspace = desk
        .engine()
        .get_report_workspace(GetReportWorkspaceInput {
            specimen_id: desk.lab.samples[0].specimen_id,
        })
        .await
        .unwrap();
    resync(&desk.lab).await;
    let source = workspace.condition_review_source;
    assert_eq!(source.conditions.len(), 4);
    let preview = workspace.preview.content.unwrap();
    assert!(
        !preview
            .specimen
            .condition_notes
            .iter()
            .any(|n| n.contains("routing evidence"))
    );
    assert_prepare_denials(&desk, &source).await;
    let candidate = prepare_with(&desk, Some(choices(&source, false)), None)
        .await
        .unwrap();
    assert!(
        candidate
            .content
            .specimen
            .condition_notes
            .iter()
            .any(|n| n.contains("accepted conditionally"))
    );
    assert!(
        !candidate
            .content
            .specimen
            .condition_notes
            .iter()
            .any(|n| n.contains("routing evidence"))
    );
    assert_raw_retained(&desk, candidate.candidate_id);
    let first = desk.release(&candidate).await.unwrap();
    let old_bytes = exported(&desk, first.revision_id).await;
    let correction = prepare_with(
        &desk,
        Some(choices(&source, true)),
        Some("Include relevant captured condition history"),
    )
    .await
    .unwrap();
    assert_eq!(correction.relationship, ReportRelationship::Correction);
    assert!(
        correction
            .content
            .specimen
            .condition_notes
            .iter()
            .any(|n| n.contains("seal leakage") && n.contains("held"))
    );
    assert!(
        !correction
            .content
            .specimen
            .condition_notes
            .iter()
            .any(|n| n.contains("routing evidence"))
    );
    desk.release(&correction).await.unwrap();
    assert_eq!(exported(&desk, first.revision_id).await, old_bytes);
}

async fn assert_prepare_denials(desk: &Desk, source: &ConditionReviewSource) {
    assert_eq!(
        prepare_with(desk, None, None).await.unwrap_err().code(),
        ErrorCode::Validation
    );
    let mut stale = choices(source, false);
    stale.source_sha256 = "0".repeat(64);
    assert_eq!(
        prepare_with(desk, Some(stale), None)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Validation
    );
    desk.lab.sign_in(Role::Accessioner).await;
    let denied = desk
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            specimen_id: desk.lab.samples[0].specimen_id,
            condition_review: Some(choices(source, false)),
            amendment_reason: None,
        })
        .await
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
}

fn assert_raw_retained(desk: &Desk, candidate_id: CandidateId) {
    let content: String = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT content_json FROM rpt_candidates WHERE id = ?1",
            [candidate_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    let frozen: lis_reporting::snapshot::ReportSnapshot = serde_json::from_str(&content).unwrap();
    assert_eq!(frozen.specimen.condition_history.len(), 4);
    assert!(frozen.specimen.condition_history.iter().any(|c| {
        c.description
            .as_deref()
            .unwrap()
            .contains("routing evidence")
    }));
    let (actor, authority, count): (String, String, i64) = desk.lab.t().conn().query_row(
        "SELECT principal_id,authority,json_extract(details, '$.condition_review_count') FROM audit_events WHERE action = 'report.prepare_candidate' AND record_id = ?1",
        [candidate_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    let expected_actor: String = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT id FROM auth_users WHERE username = 'rel1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(actor, expected_actor);
    assert_eq!(authority, "release_reports");
    assert_eq!(count, 4);
    assert_eq!(
        frozen.specimen.condition_report_review.unwrap().reason,
        "Synthetic reportability assessment; source remains internal"
    );
}
