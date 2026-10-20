//! Collection corrections preserve review/report evidence in both commit orders.
use crate::m2_lab::Role;
use crate::m2_steps::line;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::m3_report::open_keys;
use lis_engine::{
    CorrectCollectionTimeInput, FindContainerInput, ResolveCollectionCorrectionInput,
};
use lis_specimens::{CollectionHistory, CollectionTimeInput};

async fn correct(bench: &Bench) -> CollectionHistory {
    bench.sign_in(Staff::Desk(Role::Accessioner)).await;
    let engine = bench.lab().engine();
    let accession_id = engine
        .find_container(FindContainerInput {
            label_code: bench.sample().label_code.clone(),
        })
        .await
        .unwrap()
        .accession_id;
    let history = engine
        .correct_collection_time(CorrectCollectionTimeInput {
            accession_id,
            expected_revision: 1,
            time: CollectionTimeInput::Date {
                date: "2026-10-11".into(),
            },
            source_document_ref: "SYN-CORRECTED-COLLECTION".into(),
            reason: "Synthetic source confirms prior-day collection".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        bench.lab().t().events("access").last(),
        Some(&(
            "specimen.correct_collection_time".into(),
            "succeeded".into()
        ))
    );
    assert!(
        history.corrections[0].allowed_actions.is_empty(),
        "accession staff cannot resolve quality review"
    );
    history
}

async fn resolve(bench: &Bench, history: &CollectionHistory) {
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    let engine = bench.lab().engine();
    let queue_input = || lis_engine::ListWorkQueueItemsInput {
        after: None,
        queue: "collection_time_review".into(),
        limit: 25,
    };
    let pending = engine
        .list_work_queue_items(queue_input())
        .await
        .unwrap()
        .items;
    assert!(
        pending
            .iter()
            .any(|item| item.item_id == history.accession_id.to_string()
                && item.item_kind == "collection_review")
    );
    let resolved = bench
        .lab()
        .engine()
        .resolve_collection_correction(ResolveCollectionCorrectionInput {
            correction_id: history.corrections[0].correction_id,
            reason: "Synthetic stability and timing assessed against lab policy".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        bench.lab().t().events("access").last(),
        Some(&(
            "specimen.resolve_collection_correction".into(),
            "succeeded".into()
        ))
    );
    assert!(
        resolved.allowed_actions.is_empty(),
        "quality alone cannot correct collection evidence"
    );
    let remaining = engine
        .list_work_queue_items(queue_input())
        .await
        .unwrap()
        .items;
    assert!(
        !remaining
            .iter()
            .any(|item| item.item_id == history.accession_id.to_string())
    );
}

#[tokio::test]
async fn collection_change_before_release_invalidates_candidate_and_requires_rereview() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let history = correct(&bench).await;
    bench.sign_in(Staff::Releaser).await;
    assert!(bench.release(&candidate).await.is_err());
    resolve(&bench, &history).await;
    let lab = bench.desk().await;
    let results = lab.results(bench.index).await;
    match &line(&results, "SYN-MOR").review_status {
        lis_results::ReviewStatus::Stale { reasons } => {
            assert!(reasons.iter().any(|r| r.code == "collection_time_changed"))
        }
        status => panic!("collection correction must invalidate technical review: {status:?}"),
    }
    lab.approve_all(bench.index).await.unwrap();
    bench.sign_in(Staff::Releaser).await;
    assert!(
        bench.release(&candidate).await.is_err(),
        "earlier snapshot cannot be released after rereview"
    );
    let replacement = bench.prepare(None).await.unwrap();
    bench.release(&replacement).await.unwrap();
}

#[tokio::test]
async fn collection_change_after_release_retains_impact_after_quality_review_and_rereview() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let released = bench.release(&candidate).await.unwrap();
    let frozen = bench.revision_row(released.revision_id);
    let history = correct(&bench).await;
    let expected_suffix = format!(
        ":collection_time_changed:{}",
        history.corrections[0].correction_id
    );
    let keys = open_keys(&bench.report(Staff::Releaser).await);
    assert!(
        keys.iter().any(|key| key.ends_with(&expected_suffix)),
        "{keys:?}"
    );
    resolve(&bench, &history).await;
    let lab = bench.desk().await;
    lab.approve_all(bench.index).await.unwrap();
    let after = open_keys(&bench.report(Staff::Releaser).await);
    assert!(
        after.iter().any(|key| key.ends_with(&expected_suffix)),
        "impact cannot disappear after rereview: {after:?}"
    );
    assert_eq!(bench.revision_row(released.revision_id), frozen);
}
