//! Same-day financial reconciliation is bound to the exact approved evidence.
use super::*;
use lis_core::Phi;
use lis_identity::ClientId;
use lis_orders::{BillingResponsibility, SetOrderBillingContextInput};

async fn two_final_samples() -> (Desk, usize, usize) {
    let mut desk = Desk::start().await;
    let first = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    let second = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    desk.lab.import(JOURNEY).await;
    desk.import_drugs(first).await;
    desk.import_drugs(second).await;
    desk.lab.accept_qc().await;
    desk.lab.approve_all(first).await.unwrap();
    desk.lab.approve_all(second).await.unwrap();
    desk.confirm_mapping().await;
    for index in [first, second] {
        desk.condition_not_met(index).await;
        desk.release_final(index).await;
    }
    (desk, first, second)
}

async fn set_bill_to(desk: &Desk, index: usize, client: ClientId) {
    let e = desk.as_staff(Staff::Author).await;
    let order_id = desk.lab.samples[index].order_id;
    let current = e
        .get_order_billing(lis_engine::GetOrderInput { order_id })
        .await
        .unwrap();
    e.set_order_billing_context(SetOrderBillingContextInput {
        order_id,
        expected_revision: current.order_revision,
        expected_billing_version: current.context.map(|context| context.version),
        responsibility: BillingResponsibility::Client,
        client_id: Some(client),
        profile_version: None,
        coverage_keys: vec![],
        use_profile_guarantor: false,
        financial_evidence: vec![],
        source: Phi::new("SYN-CLIENT-CONFIRMATION".into()),
        reason: Phi::new("Synthetic source confirms bill-to allocation".into()),
    })
    .await
    .unwrap();
}

async fn resolve(desk: &Desk, view: &DerivationView) -> Result<DerivationView, CommandError> {
    desk.as_staff(Staff::Author)
        .await
        .resolve_billing_context_conflict(lis_engine::ResolveBillingContextConflictInput {
            derivation_id: view.derivation_id,
            expected_revision: view.revision,
            reason: "Synthetic billing owner reconciled same-day allocation".into(),
        })
        .await
}

fn service_facts(
    view: &DerivationView,
) -> Vec<(
    lis_specimens::SpecimenId,
    Vec<lis_engine::DerivationClassView>,
)> {
    view.specimens
        .iter()
        .map(|specimen| (specimen.specimen_id, specimen.classes.clone()))
        .collect()
}

#[tokio::test]
async fn same_day_context_conflict_requires_current_resolution_without_duplicating_services() {
    let (desk, _, second) = two_final_samples().await;
    let original = desk.derive(Some("2026-10-11")).await;
    assert_eq!(original.state, BillingDerivationState::Candidate);
    assert_eq!(original.specimens.len(), 2);
    desk.resync().await;
    desk.lab.sign_in(Role::Admin).await;
    let client = desk
        .engine()
        .create_client(lis_engine::CreateClient {
            code: "SYN-OTHER-BILLTO".into(),
            name: "Synthetic alternate bill-to".into(),
        })
        .await
        .unwrap();
    set_bill_to(&desk, second, client).await;
    assert!(desk.derivation(&original).await.stale);
    assert_eq!(
        desk.approve(&original).await.unwrap_err().code(),
        ErrorCode::Conflict
    );
    let held = desk.derive(Some("2026-10-11")).await;
    assert!(
        held.flags
            .iter()
            .any(|flag| flag.code == "billing_context_conflict" && flag.blocking)
    );
    assert_eq!(service_facts(&held), service_facts(&original));
    assert_invalid_resolutions(&desk, &held).await;
    let resolved = resolve(&desk, &held).await.unwrap();
    assert_eq!(resolved.state, BillingDerivationState::Candidate);
    assert_eq!(service_facts(&resolved), service_facts(&original));
    assert!(resolved.context_resolution.is_some());
    let approved = desk.approve(&resolved).await.unwrap();
    assert_resolution_expires(&desk, second, client, &approved, &original).await;
}

async fn assert_invalid_resolutions(desk: &Desk, held: &DerivationView) {
    let engine = desk.as_staff(Staff::Author).await;
    for (revision, reason, expected) in [
        (held.revision + 1, "Synthetic reason", ErrorCode::Conflict),
        (held.revision, " ", ErrorCode::Validation),
    ] {
        let error = engine
            .resolve_billing_context_conflict(lis_engine::ResolveBillingContextConflictInput {
                derivation_id: held.derivation_id,
                expected_revision: revision,
                reason: reason.into(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), expected);
    }
}

async fn assert_resolution_expires(
    desk: &Desk,
    second: usize,
    client: ClientId,
    approved: &DerivationView,
    original: &DerivationView,
) {
    // Even the same destination confirmed in a new financial version needs a
    // fresh same-day reconciliation; the prior explanation covers its exact digest.
    set_bill_to(desk, second, client).await;
    assert!(desk.derivation(approved).await.stale);
    assert_eq!(
        desk.hand_off(&[approved]).await.unwrap_err().code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        resolve(desk, approved).await.unwrap_err().code(),
        ErrorCode::Conflict
    );
    let revised = desk.derive(Some("2026-10-11")).await;
    assert_unresolved_conflict(&revised, original);
    let resolved = resolve(desk, &revised).await.unwrap();
    let approved = desk.approve(&resolved).await.unwrap();
    let handoff = desk.hand_off(&[&approved]).await.unwrap();
    assert_eq!(handoff.items.len(), 1);
    assert_eq!(desk.count("SELECT count(*) FROM audit_events WHERE stream = 'clinical' AND action = 'billing.resolve_context_conflict' AND outcome = 'succeeded'"), 2);
}

fn assert_unresolved_conflict(view: &DerivationView, original: &DerivationView) {
    assert!(view.context_resolution.is_none());
    assert!(
        view.flags
            .iter()
            .any(|flag| flag.code == "billing_context_conflict" && flag.blocking)
    );
    assert_eq!(service_facts(view), service_facts(original));
}
