//! Synthetic financial fixture and billing-only intake through engine commands.
use lis_core::{ErrorCode, Phi};
use lis_engine::{Engine, GetOrderInput};
use lis_identity::{
    BillingContact, GetPatientBillingProfileInput, PatientBillingContent,
    SetPatientBillingProfileInput,
};
use lis_orders::{BillingResponsibility, OrderId, SetOrderBillingContextInput};

pub(crate) async fn confirm_client(engine: &Engine, order_id: OrderId) {
    let order = engine.get_order(GetOrderInput { order_id }).await.unwrap();
    engine
        .set_order_billing_context(SetOrderBillingContextInput {
            order_id,
            expected_revision: order.revision,
            expected_billing_version: None,
            responsibility: BillingResponsibility::Client,
            client_id: Some(order.client_id),
            profile_version: None,
            coverage_keys: vec![],
            use_profile_guarantor: false,
            financial_evidence: vec![],
            source: Phi::new("SYN-CLIENT-CONTRACT".into()),
            reason: Phi::new("Synthetic client billing confirmed".into()),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn billing_only_can_capture_financial_context_without_clinical_order_access() {
    let lab = crate::m2_lab::Lab::start(lis_analytical::RevisionIndicator::None).await;
    lab.sign_in(crate::m2_lab::Role::Admin).await;
    lab.t()
        .create_user("finance", &[lis_auth::RoleCode::Billing])
        .await;
    lab.t().first_sign_in("finance").await;
    let engine = lab.engine();
    let subject_id = lab.parties.subject;
    let order_id = lab.samples[0].order_id;
    assert_eq!(
        engine
            .get_order(GetOrderInput { order_id })
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
    let content = patient_content();
    let first = engine
        .set_patient_billing_profile(SetPatientBillingProfileInput {
            subject_id,
            expected_version: None,
            content: content.clone(),
            source: Phi::new("Synthetic patient attestation".into()),
            reason: Phi::new("Initial supplied address".into()),
        })
        .await
        .unwrap();
    let view = engine
        .get_order_billing(GetOrderInput { order_id })
        .await
        .unwrap();
    assert_eq!(view.subject.subject_id, subject_id);
    let selected = engine
        .set_order_billing_context(SetOrderBillingContextInput {
            order_id,
            expected_revision: view.order_revision,
            expected_billing_version: None,
            responsibility: BillingResponsibility::PatientSelfPay,
            client_id: None,
            profile_version: Some(first.version),
            coverage_keys: vec![],
            use_profile_guarantor: false,
            financial_evidence: vec![],
            source: Phi::new("Synthetic request".into()),
            reason: Phi::new("Self-pay explicitly selected".into()),
        })
        .await
        .unwrap();
    let mut changed = content;
    changed.contact.city = Some(Phi::new("Changed City".into()));
    engine
        .set_patient_billing_profile(SetPatientBillingProfileInput {
            subject_id,
            expected_version: Some(first.version),
            content: changed,
            source: Phi::new("Synthetic update".into()),
            reason: Phi::new("Later address change".into()),
        })
        .await
        .unwrap();
    let historical = engine
        .get_patient_billing_profile(GetPatientBillingProfileInput {
            subject_id,
            version: Some(1),
        })
        .await
        .unwrap()
        .profile
        .unwrap();
    assert_eq!(historical.content, first.content);
    let retained = engine
        .get_order_billing(GetOrderInput { order_id })
        .await
        .unwrap();
    assert_eq!(retained.context.unwrap().selection, selected.selection);
    assert!(retained.readiness_issues.is_empty());
    followup_as_billing(engine, order_id, subject_id).await;
    assert_financial_write_access(&lab);
}

fn assert_financial_write_access(lab: &crate::m2_lab::Lab) {
    let access = lab.t().events("access");
    for (action, expected) in [
        ("identity.set_billing_profile", 2),
        ("order.set_billing_context", 1),
        ("order.billing_review", 1),
        ("order.update_billing_review", 1),
    ] {
        assert_eq!(
            access
                .iter()
                .filter(|(name, outcome)| name == action && outcome == "succeeded")
                .count(),
            expected,
            "each sensitive write response must record access: {action}"
        );
    }
}

fn patient_content() -> PatientBillingContent {
    PatientBillingContent {
        contact: BillingContact {
            address_lines: vec![Phi::new("100 Synthetic Lane".into())],
            city: Some(Phi::new("Test City".into())),
            region: Some(Phi::new("NY".into())),
            postal_code: Some(Phi::new("10000".into())),
            country: Some("US".into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

async fn followup_as_billing(
    engine: &Engine,
    order_id: OrderId,
    subject_id: lis_identity::SubjectId,
) {
    let opened = engine
        .create_billing_review_item(lis_orders::CreateBillingReviewItemInput {
            order_id,
            element: lis_orders::BillingReviewElement::Other,
            note: Some("Synthetic requested evidence follow-up".into()),
        })
        .await
        .unwrap();
    let item = &opened.review_items[0];
    assert_eq!(opened.subject.subject_id, subject_id);
    let queue = engine
        .list_billing_review_items(lis_orders::ListBillingReviewItemsInput {
            status: Some(lis_orders::BillingReviewStatus::Open),
            limit: 25,
            after_item_id: None,
        })
        .await
        .unwrap();
    assert!(queue.iter().any(|entry| entry.id == item.id));
    engine
        .update_billing_review_item(lis_orders::UpdateBillingReviewItemInput {
            item_id: item.id,
            expected_version: item.version,
            status: lis_orders::BillingReviewStatus::Resolved,
            owner: Some("Synthetic billing desk".into()),
            source_ref: Some(Phi::new("SYN-RESPONSE-1".into())),
            reason: Phi::new("Requested evidence received".into()),
        })
        .await
        .unwrap();
    let resolved = engine
        .get_order_billing(GetOrderInput { order_id })
        .await
        .unwrap();
    assert!(resolved.readiness_issues.is_empty());
}
