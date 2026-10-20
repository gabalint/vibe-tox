//! BILL-G02/G07: order-selected profile history and separate financial readiness.

use lis_core::{Capability, CorrelationId, ErrorCode, Phi};
use lis_identity::{
    BillingContact, BillingParty, BillingRelationship, FinancialEvidence, FinancialEvidenceKind,
    FinancialEvidenceStatus, PatientBillingContent, PatientCoverage, SetPatientBillingProfileInput,
    set_patient_billing_profile,
};
use lis_orders::{
    AcceptOrderInput, BillingResponsibility, OrderBillingContext, OrderDetail, OrderError,
    SetOrderBillingContextInput, accept_order, billing_context_readiness,
    get_order_billing_context, order_billing_view, set_order_billing_context,
};
use lis_store::testing::principal;

use crate::Fixture;

fn profile_input(fixture: &Fixture) -> SetPatientBillingProfileInput {
    let contact = BillingContact {
        address_lines: vec![Phi::new("1 Synthetic Way".into())],
        city: Some(Phi::new("Synthetic City".into())),
        postal_code: Some(Phi::new("00000".into())),
        ..BillingContact::default()
    };
    let subscriber = BillingParty {
        name: Phi::new("Synthetic parent".into()),
        relationship: BillingRelationship::Child,
        birth_date: Some(Phi::new("1970-01-01".into())),
        contact: BillingContact::default(),
    };
    let guarantor = BillingParty {
        name: Phi::new("Synthetic guardian".into()),
        relationship: BillingRelationship::Other,
        birth_date: None,
        contact: contact.clone(),
    };
    SetPatientBillingProfileInput {
        subject_id: fixture.subject_id,
        expected_version: None,
        content: PatientBillingContent {
            contact,
            guarantor: Some(guarantor),
            coverages: vec![PatientCoverage {
                key: "primary".into(),
                payer_name: "Synthetic Plan".into(),
                payer_id: Some("SYN".into()),
                plan_name: None,
                member_id: Some(Phi::new("CANARY-MEMBER-OLD".into())),
                group_number: None,
                subscriber: Some(subscriber),
                effective_from: None,
                effective_until: None,
                source: Phi::new("retained-card".into()),
                verified_on: None,
            }],
            financial_evidence: vec![],
        },
        source: Phi::new("synthetic requisition".into()),
        reason: Phi::new("supplied finance".into()),
    }
}

fn save_profile(fixture: &Fixture, input: SetPatientBillingProfileInput) {
    fixture
        .t
        .db
        .write_blocking(
            fixture.orderer.clone(),
            CorrelationId::generate(),
            move |ctx| set_patient_billing_profile(ctx, input),
        )
        .unwrap();
}

fn context_input(order: &OrderDetail) -> SetOrderBillingContextInput {
    SetOrderBillingContextInput {
        order_id: order.order_id,
        expected_revision: order.revision,
        expected_billing_version: None,
        responsibility: BillingResponsibility::ThirdParty,
        client_id: None,
        profile_version: Some(1),
        coverage_keys: vec!["primary".into()],
        use_profile_guarantor: true,
        financial_evidence: vec![],
        source: Phi::new("synthetic financial source".into()),
        reason: Phi::new("confirmed for order".into()),
    }
}

fn get_context(fixture: &Fixture, order: &OrderDetail) -> OrderBillingContext {
    let id = order.order_id;
    fixture
        .t
        .db
        .read_blocking(move |conn| get_order_billing_context(conn, id))
        .unwrap()
        .unwrap()
}

#[test]
fn order_snapshot_keeps_subscriber_and_guarantor_distinct_after_patient_coverage_changes() {
    let fixture = Fixture::open();
    save_profile(&fixture, profile_input(&fixture));
    let order = fixture.register(None);
    let input = context_input(&order);
    let context = fixture
        .write(move |ctx| set_order_billing_context(ctx, input))
        .unwrap();
    assert!(billing_context_readiness(Some(&context)).is_empty());
    assert_ne!(
        context.selection.guarantor.as_ref().unwrap().name,
        context.selection.coverages[0]
            .subscriber
            .as_ref()
            .unwrap()
            .name
    );
    let mut profile = profile_input(&fixture);
    profile.expected_version = Some(1);
    profile.content.coverages[0].member_id = Some(Phi::new("NEW-MEMBER".into()));
    save_profile(&fixture, profile);
    let historical = get_context(&fixture, &order);
    assert_eq!(
        historical.selection.coverages[0]
            .member_id
            .as_ref()
            .unwrap()
            .expose(),
        "CANARY-MEMBER-OLD"
    );
    assert_eq!(historical.selection.profile_version, Some(1));
    let id = order.order_id;
    let detail = fixture
        .t
        .db
        .read_blocking(move |conn| lis_orders::get_order(conn, id))
        .unwrap();
    assert_eq!(detail.version, order.version + 1);
    assert_eq!(detail.revision, order.revision + 1);
    assert_eq!(
        detail.evidence.authority.len(),
        order.evidence.authority.len()
    );
}

#[test]
fn a_billing_only_user_can_correct_bill_to_without_changing_clinical_scope() {
    let fixture = Fixture::open();
    save_profile(&fixture, profile_input(&fixture));
    let order = fixture.register(None);
    let input = context_input(&order);
    fixture
        .write(move |ctx| set_order_billing_context(ctx, input))
        .unwrap();
    let mut correction = context_input(&order);
    correction.expected_revision += 1;
    correction.expected_billing_version = Some(1);
    correction.responsibility = BillingResponsibility::Client;
    correction.client_id = Some(fixture.client_id);
    correction.coverage_keys.clear();
    let changed = fixture
        .t
        .db
        .write_blocking(
            principal(&[Capability::Billing]),
            CorrelationId::generate(),
            move |ctx| set_order_billing_context(ctx, correction),
        )
        .unwrap();
    assert_eq!(changed.version, 2);
    let id = order.order_id;
    let old: String = fixture
        .t
        .db
        .read_blocking(move |conn| {
            conn.query_row(
                "SELECT context_json FROM ord_billing_contexts WHERE order_id=?1 AND version=1",
                [id],
                |row| row.get(0),
            )
            .map_err(OrderError::from)
        })
        .unwrap();
    assert!(old.contains("CANARY-MEMBER-OLD"));
    let detail = fixture
        .t
        .db
        .read_blocking(move |conn| lis_orders::get_order(conn, id))
        .unwrap();
    assert_eq!(detail.billing_responsibility, BillingResponsibility::Client);
    assert_eq!(detail.items.len(), order.items.len());
    let view = fixture
        .t
        .db
        .read_blocking(move |conn| order_billing_view(conn, &principal(&[Capability::Billing]), id))
        .unwrap();
    assert_eq!(view.subject.subject_id, fixture.subject_id);
    assert_eq!(view.order_revision, order.revision + 2);
}

#[test]
fn missing_finance_never_blocks_clinically_authorized_order_acceptance() {
    let fixture = Fixture::open();
    let order = fixture.register(None);
    let mut input = context_input(&order);
    input.profile_version = None;
    input.coverage_keys.clear();
    input.use_profile_guarantor = false;
    input.responsibility = BillingResponsibility::NotSupplied;
    let context = fixture
        .write(move |ctx| set_order_billing_context(ctx, input))
        .unwrap();
    assert_eq!(
        billing_context_readiness(Some(&context)),
        ["billing_responsibility_missing"]
    );
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: order.order_id,
                    expected_revision: order.revision + 1,
                },
            )
        })
        .unwrap();
    assert_eq!(accepted.status, lis_orders::OrderStatus::Accepted);
    assert!(accepted.holds.is_empty());
}

#[test]
fn stale_replay_unauthorized_and_malformed_contexts_are_refused() {
    let fixture = Fixture::open();
    save_profile(&fixture, profile_input(&fixture));
    let order = fixture.register(None);
    let initial = context_input(&order);
    fixture
        .write(move |ctx| set_order_billing_context(ctx, initial))
        .unwrap();
    let stale = context_input(&order);
    assert!(matches!(
        fixture.write(move |ctx| set_order_billing_context(ctx, stale)),
        Err(OrderError::RevisionConflict)
    ));
    let unauthorized = context_input(&order);
    let denied = fixture
        .t
        .db
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| set_order_billing_context(ctx, unauthorized),
        )
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
    assert!(matches!(
        denied,
        OrderError::Store(lis_store::StoreError::Denied(_))
    ));
    let mut wrong = context_input(&order);
    wrong.expected_revision += 1;
    wrong.expected_billing_version = Some(1);
    wrong.coverage_keys = vec!["missing".into()];
    assert_eq!(
        fixture
            .write(move |ctx| set_order_billing_context(ctx, wrong))
            .unwrap_err()
            .code(),
        ErrorCode::Validation
    );
    assert_eq!(get_context(&fixture, &order).version, 1);
}

#[test]
fn invalid_manual_evidence_preserves_field_feedback_and_writes_no_snapshot() {
    let fixture = Fixture::open();
    save_profile(&fixture, profile_input(&fixture));
    let order = fixture.register(None);
    let mut input = context_input(&order);
    input.financial_evidence.push(FinancialEvidence {
        key: "eligibility-check".into(),
        kind: FinancialEvidenceKind::Eligibility,
        status: FinancialEvidenceStatus::Recorded,
        source_ref: Phi::new("SYN-ELIGIBILITY-SOURCE".into()),
        scope: Phi::new(" ".into()),
        occurred_at: lis_store::testing::test_start(),
        valid_from: None,
        valid_until: None,
        reference_number: None,
        coverage_key: Some("primary".into()),
        provider_npi: None,
        units: None,
        notice: None,
        notes: None,
    });
    let error = fixture
        .write(move |ctx| set_order_billing_context(ctx, input))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    assert_eq!(error.field_errors().len(), 1);
    assert_eq!(error.field_errors()[0].field, "scope");
    assert_eq!(error.field_errors()[0].code, "required");
    let id = order.order_id;
    assert!(
        fixture
            .t
            .db
            .read_blocking(move |conn| get_order_billing_context(conn, id))
            .unwrap()
            .is_none()
    );
}

#[test]
fn profile_selection_cannot_cross_patient_identity_or_select_duplicate_coverages() {
    let fixture = Fixture::open();
    let other = fixture
        .t
        .db
        .write_blocking(fixture.orderer.clone(), CorrelationId::generate(), |ctx| {
            lis_identity::register_subject(
                ctx,
                lis_identity::RegisterSubject {
                    demographics: lis_identity::DemographicsInput {
                        family_name: Phi::new("Anotherfinancetrail".into()),
                        given_names: Some(Phi::new("Synthetic".into())),
                        birth_date: Phi::new("1990-01-01".into()),
                        administrative_sex: None,
                    },
                    source: "synthetic".into(),
                    identifiers: vec![],
                    acknowledged_candidates: vec![],
                },
            )
        })
        .unwrap();
    let mut other_profile = profile_input(&fixture);
    other_profile.subject_id = other.subject_id;
    save_profile(&fixture, other_profile);
    let order = fixture.register(None);
    let input = context_input(&order);
    assert_eq!(
        fixture
            .write(move |ctx| set_order_billing_context(ctx, input))
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    save_profile(&fixture, profile_input(&fixture));
    let mut duplicate = context_input(&order);
    duplicate.coverage_keys.push("primary".into());
    assert_eq!(
        fixture
            .write(move |ctx| set_order_billing_context(ctx, duplicate))
            .unwrap_err()
            .code(),
        ErrorCode::Validation
    );
}

#[test]
fn context_versions_are_immutable_and_audits_exclude_member_details() {
    let fixture = Fixture::open();
    save_profile(&fixture, profile_input(&fixture));
    let order = fixture.register(None);
    let input = context_input(&order);
    let saved = fixture
        .write(move |ctx| set_order_billing_context(ctx, input))
        .unwrap();
    assert!(!format!("{saved:?}").contains("CANARY-MEMBER-OLD"));
    for sql in [
        "UPDATE ord_billing_contexts SET context_json='{}'",
        "DELETE FROM ord_billing_contexts",
    ] {
        assert!(
            fixture
                .write(move |ctx| Ok(ctx.tx().execute(sql, [])?))
                .is_err()
        );
    }
    let (action_count, leaked, reason): (i64, bool, Option<String>) = fixture.t.db.read_blocking(|conn| {
        conn.query_row("SELECT count(*),coalesce(max(instr(details,'CANARY-MEMBER-OLD') > 0),0),max(reason) FROM audit_events WHERE action='order.set_billing_context'", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(OrderError::from)
    }).unwrap();
    assert_eq!(action_count, 1);
    assert!(!leaked);
    assert_eq!(reason.as_deref(), Some("confirmed for order"));
}
