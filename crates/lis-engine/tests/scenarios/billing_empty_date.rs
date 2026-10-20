//! Corrected, never-exported dates close without creating a zero-service original.
use super::*;

#[tokio::test]
async fn corrected_unknown_date_supersedes_unexported_group_without_stranding_work() {
    let mut desk = Desk::start().await;
    let index = desk
        .add_sample(
            desk.lab.orderables.panel.1,
            CollectionTimeInput::Unknown,
            false,
        )
        .await;
    let original = desk.derive(None).await;
    assert_eq!(original.state, BillingDerivationState::Held);
    desk.resync().await;
    desk.lab.sign_in(Role::Accessioner).await;
    let accession = desk
        .engine()
        .find_container(lis_engine::FindContainerInput {
            label_code: desk.lab.samples[index].label_code.clone(),
        })
        .await
        .unwrap();
    desk.engine()
        .correct_collection_time(lis_engine::CorrectCollectionTimeInput {
            accession_id: accession.accession_id,
            expected_revision: 1,
            time: CollectionTimeInput::Date {
                date: "2026-10-11".into(),
            },
            source_document_ref: "SYN-CONFIRMED-DATE".into(),
            reason: "Synthetic source supplied missing collection date".into(),
        })
        .await
        .unwrap();
    let closed = desk.derive(None).await;
    assert_eq!(closed.derivation_id, original.derivation_id);
    assert_eq!(closed.state, BillingDerivationState::Superseded);
    assert!(
        desk.work(BillingWorkFilter::NeedsDerivation)
            .await
            .iter()
            .all(|item| item.date_of_service.is_some())
    );
    assert_eq!(desk.derive(None).await.revision, closed.revision);
    assert_eq!(
        desk.hand_off(&[&closed]).await.unwrap_err().code(),
        ErrorCode::Conflict
    );
    assert_eq!(desk.count("SELECT count(*) FROM bil_handoffs"), 0);
}
