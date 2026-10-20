//! A corrected collection date reconciles both service days without losing the prior export.
use super::*;
use lis_engine::{
    CorrectCollectionTimeInput, FindContainerInput, ResolveCollectionCorrectionInput,
};

#[tokio::test]
async fn corrected_date_voids_old_day_and_rederives_new_day_after_clinical_review() {
    let mut desk = Desk::start().await;
    let index = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    desk.lab.import(JOURNEY).await;
    desk.import_drugs(index).await;
    desk.lab.accept_qc().await;
    desk.lab.approve_all(index).await.unwrap();
    desk.confirm_mapping().await;
    desk.condition_not_met(index).await;
    desk.release_final(index).await;
    let original = desk.derive(Some("2026-10-11")).await;
    let original = desk.approve(&original).await.unwrap();
    let first_handoff = desk.hand_off(&[&original]).await.unwrap();

    desk.resync().await;
    desk.lab.sign_in(Role::Accessioner).await;
    let accession_id = desk
        .engine()
        .find_container(FindContainerInput {
            label_code: desk.lab.samples[index].label_code.clone(),
        })
        .await
        .unwrap()
        .accession_id;
    let correction = desk
        .engine()
        .correct_collection_time(CorrectCollectionTimeInput {
            accession_id,
            expected_revision: 1,
            time: CollectionTimeInput::Date {
                date: "2026-10-10".into(),
            },
            source_document_ref: "SYN-SOURCE-DATE-CORRECTION".into(),
            reason: "Synthetic source confirms October 10".into(),
        })
        .await
        .unwrap();
    let work = desk.work(BillingWorkFilter::NeedsDerivation).await;
    for date in ["2026-10-10", "2026-10-11"] {
        assert!(
            work.iter()
                .any(|item| item.date_of_service.as_deref() == Some(date)),
            "{date}: {work:?}"
        );
    }
    let old_day = desk.derive(Some("2026-10-11")).await;
    assert!(old_day.specimens.is_empty());
    assert_eq!(adjusted(&old_day), Some(original.derivation_id));
    let old_day = desk.approve(&old_day).await.unwrap();
    let adjustment = desk.hand_off(&[&old_day]).await.unwrap();
    assert_eq!(adjustment.items[0].kind, HandoffItemKind::Adjustment);
    assert_void_export(&desk, &adjustment, &first_handoff).await;
    let new_day = desk.derive(Some("2026-10-10")).await;
    assert_eq!(new_day.state, BillingDerivationState::Held);

    amend_after_correction(&desk, index, &correction).await;
    let new_day = desk.derive(Some("2026-10-10")).await;
    assert_eq!(
        new_day.state,
        BillingDerivationState::Candidate,
        "{:?}",
        new_day.flags
    );
    assert_eq!(new_day.specimens.len(), 1);
    assert!(
        new_day.adjustment_of.is_none(),
        "new date is a distinct service group"
    );
    let new_day = desk.approve(&new_day).await.unwrap();
    desk.hand_off(&[&new_day]).await.unwrap();
}

async fn amend_after_correction(
    desk: &Desk,
    index: usize,
    correction: &lis_specimens::CollectionHistory,
) {
    desk.resync().await;
    desk.engine()
        .resolve_collection_correction(ResolveCollectionCorrectionInput {
            correction_id: correction.corrections[0].correction_id,
            reason: "Synthetic quality timing assessment completed".into(),
        })
        .await
        .unwrap();
    desk.lab.approve_all(index).await.unwrap();
    let e = desk.as_staff(Staff::Releaser).await;
    let amended = e
        .prepare_report_candidate(PrepareReportCandidateInput {
            condition_review: None,
            specimen_id: desk.lab.samples[index].specimen_id,
            amendment_reason: Some("Synthetic corrected collection date".into()),
        })
        .await
        .unwrap();
    e.release_report(ReleaseReportInput {
        candidate_id: amended.candidate_id,
        expected_revision: amended.revision,
        content_sha256: amended.content_sha256.clone(),
        acknowledge_partial: false,
        password: secret(NEW_PASSWORD).into(),
        self_approval: false,
    })
    .await
    .unwrap();
}

async fn assert_void_export(desk: &Desk, adjustment: &HandoffView, first_handoff: &HandoffView) {
    let downloads = tempfile::tempdir().unwrap();
    let exported = desk
        .engine()
        .export_billing_handoff(
            BillingHandoffInput {
                handoff_id: adjustment.handoff_id,
            },
            Some(downloads.path().to_path_buf()),
        )
        .await
        .unwrap();
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(downloads.path().join(exported.file_name)).unwrap())
            .unwrap();
    assert_eq!(file["items"][0]["voidsPriorServices"], true);
    assert_eq!(
        file["items"][0]["adjustsItemId"],
        first_handoff.items[0].item_id.to_string()
    );
}
