//! Financial intake command authorization and collection/report interaction.
use lis_core::{CommandError, Phi};
use lis_engine::{Engine, GetAccessionInput, GetOrderInput};
use lis_identity::{
    GetPatientBillingProfileInput, PatientBillingContent, SetPatientBillingProfileInput, SubjectId,
};
use lis_orders::{
    BillingResponsibility, BillingReviewItemId, BillingReviewStatus, OrderId,
    SetOrderBillingContextInput, UpdateBillingReviewItemInput,
};
use lis_specimens::{
    AccessionId, CollectionCorrectionId, CollectionTimeInput, CorrectCollectionTimeInput,
    ResolveCollectionCorrectionInput,
};

pub(crate) async fn billing_intake_call(
    engine: &Engine,
    name: &str,
) -> Option<Result<(), CommandError>> {
    let subject_id = SubjectId::generate();
    let order_id = OrderId::generate();
    Some(match name {
        "get_patient_billing_profile" => engine
            .get_patient_billing_profile(GetPatientBillingProfileInput {
                subject_id,
                version: None,
            })
            .await
            .map(drop),
        "set_patient_billing_profile" => engine
            .set_patient_billing_profile(SetPatientBillingProfileInput {
                subject_id,
                expected_version: None,
                content: PatientBillingContent::default(),
                source: Phi::new("Synthetic intake".into()),
                reason: Phi::new("Synthetic intake".into()),
            })
            .await
            .map(drop),
        "get_order_billing" => engine
            .get_order_billing(GetOrderInput { order_id })
            .await
            .map(drop),
        "set_order_billing_context" => engine
            .set_order_billing_context(SetOrderBillingContextInput {
                order_id,
                expected_revision: 1,
                expected_billing_version: None,
                responsibility: BillingResponsibility::PatientSelfPay,
                client_id: None,
                profile_version: None,
                coverage_keys: vec![],
                use_profile_guarantor: false,
                financial_evidence: vec![],
                source: Phi::new("Synthetic intake".into()),
                reason: Phi::new("Synthetic confirmation".into()),
            })
            .await
            .map(drop),
        "update_billing_review_item" => engine
            .update_billing_review_item(UpdateBillingReviewItemInput {
                item_id: BillingReviewItemId::generate(),
                expected_version: 1,
                status: BillingReviewStatus::Open,
                owner: None,
                source_ref: None,
                reason: Phi::new("Synthetic follow-up".into()),
            })
            .await
            .map(drop),
        _ => return reconciliation_call(engine, name).await,
    })
}

async fn collection_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let accession_id = AccessionId::generate();
    Some(match name {
        "get_collection_history" => engine
            .get_collection_history(GetAccessionInput { accession_id })
            .await
            .map(drop),
        "correct_collection_time" => engine
            .correct_collection_time(CorrectCollectionTimeInput {
                accession_id,
                expected_revision: 1,
                time: CollectionTimeInput::Date {
                    date: "2026-10-12".into(),
                },
                source_document_ref: "Synthetic corrected requisition".into(),
                reason: "Synthetic transcription correction".into(),
            })
            .await
            .map(drop),
        "resolve_collection_correction" => engine
            .resolve_collection_correction(ResolveCollectionCorrectionInput {
                correction_id: CollectionCorrectionId::generate(),
                reason: "Synthetic quality review".into(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn reconciliation_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "list_billing_review_items" => engine
            .list_billing_review_items(lis_orders::ListBillingReviewItemsInput {
                status: None,
                limit: 25,
                after_item_id: None,
            })
            .await
            .map(drop),
        "resolve_billing_context_conflict" => engine
            .resolve_billing_context_conflict(lis_billing::ResolveBillingContextConflictInput {
                derivation_id: lis_billing::BillingDerivationId::generate(),
                expected_revision: 1,
                reason: "Synthetic reconciliation".into(),
            })
            .await
            .map(drop),
        "record_handoff_followup" => engine
            .record_handoff_followup(lis_billing::RecordHandoffFollowupInput {
                item_id: lis_billing::BillingHandoffItemId::generate(),
                expected_revision: 1,
                outcome: lis_billing::HandoffOutcome::Acknowledged,
                external_reference: None,
                reason: "Synthetic billing-owner response".into(),
            })
            .await
            .map(drop),
        _ => return collection_call(engine, name).await,
    })
}
