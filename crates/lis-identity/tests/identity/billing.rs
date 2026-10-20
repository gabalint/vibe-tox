//! BILL-G01/G07: versioned coverage, role separation and PHI-safe manual evidence.

use lis_core::{Capability, ErrorCode, Phi, Principal};
use lis_identity::{
    BillingContact, BillingParty, BillingRelationship, FinancialEvidence, FinancialEvidenceKind,
    FinancialEvidenceStatus, GetPatientBillingProfileInput, PatientBillingContent, PatientCoverage,
    SetPatientBillingProfileInput, SubjectId, get_patient_billing_profile, patient_billing_view,
    set_patient_billing_profile,
};
use lis_store::testing::{TestDatabase, principal, test_start};

use crate::{assert_code, counts, demographics, open, read, register, run, run_as};

fn fixture() -> (TestDatabase, SubjectId) {
    let t = open();
    let patient = register(
        &t,
        demographics("Financetrail", "Synthetic", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    (t, patient.subject_id)
}

fn input(subject_id: SubjectId) -> SetPatientBillingProfileInput {
    let party = BillingParty {
        name: Phi::new("Synthetic subscriber".into()),
        relationship: BillingRelationship::Child,
        birth_date: Some(Phi::new("1970-01-01".into())),
        contact: BillingContact::default(),
    };
    let coverage = PatientCoverage {
        key: "plan-a".into(),
        payer_name: "Synthetic payer".into(),
        payer_id: Some("payer-a".into()),
        plan_name: None,
        member_id: Some(Phi::new("CANARY-MEMBER".into())),
        group_number: None,
        subscriber: Some(party),
        effective_from: Some("2026-01-01".into()),
        effective_until: None,
        source: Phi::new("synthetic-card-reference".into()),
        verified_on: None,
    };
    SetPatientBillingProfileInput {
        subject_id,
        expected_version: None,
        content: PatientBillingContent {
            coverages: vec![coverage],
            ..PatientBillingContent::default()
        },
        source: Phi::new("synthetic intake".into()),
        reason: Phi::new("initial supplied profile".into()),
    }
}

#[test]
fn billing_profile_retains_coverage_history_without_overwriting_identity() {
    let (t, patient) = fixture();
    let first = run(&t, &[Capability::Billing], move |ctx| {
        set_patient_billing_profile(ctx, input(patient))
    })
    .unwrap();
    let mut change = input(patient);
    change.expected_version = Some(1);
    change.content.coverages[0].member_id = Some(Phi::new("NEW-MEMBER".into()));
    change.reason = Phi::new("Source supplied corrected coverage".into());
    let second = run(&t, &[Capability::EnterOrders], move |ctx| {
        set_patient_billing_profile(ctx, change)
    })
    .unwrap();
    assert_eq!(second.version, 2);
    let historical = read(&t, move |conn| {
        get_patient_billing_profile(conn, patient, Some(1))
    })
    .unwrap();
    assert_eq!(historical.content, first.content);
    assert_eq!(
        read(&t, move |conn| lis_identity::subject_summary(conn, patient)).revision,
        1
    );
    let reasons: Vec<String> = read(&t, |conn| {
        let mut statement = conn.prepare(
            "SELECT reason FROM audit_events WHERE action = 'identity.set_patient_billing_profile' ORDER BY seq",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    });
    assert_eq!(
        reasons,
        [
            "initial supplied profile",
            "Source supplied corrected coverage"
        ]
    );
    let authorities: Vec<String> = read(&t, |conn| {
        let mut statement = conn.prepare(
            "SELECT authority FROM audit_events WHERE action = 'identity.set_patient_billing_profile' ORDER BY seq",
        )?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    });
    assert_eq!(authorities, ["billing", "enter_orders"]);
    assert!(!format!("{first:?}").contains("CANARY-MEMBER"));
    let audit: String = read(&t, |conn| {
        Ok(conn.query_row(
            "SELECT group_concat(details) FROM audit_events",
            [],
            |row| row.get(0),
        )?)
    });
    assert!(!audit.contains("CANARY-MEMBER"));
}

#[test]
fn stale_profile_replay_and_unauthorized_updates_write_nothing() {
    let (t, patient) = fixture();
    run(&t, &[Capability::Billing], move |ctx| {
        set_patient_billing_profile(ctx, input(patient))
    })
    .unwrap();
    let before = counts(&t);
    assert_code(
        run(&t, &[Capability::Billing], move |ctx| {
            set_patient_billing_profile(ctx, input(patient))
        }),
        ErrorCode::Conflict,
    );
    assert_code(
        run(&t, &[Capability::ManageConfiguration], move |ctx| {
            set_patient_billing_profile(ctx, input(patient))
        }),
        ErrorCode::Forbidden,
    );
    assert_code(
        run_as(
            &t,
            Principal::system("synthetic", [Capability::Billing]),
            move |ctx| set_patient_billing_profile(ctx, input(patient)),
        ),
        ErrorCode::Forbidden,
    );
    assert_eq!(counts(&t), before);
}

#[test]
fn profile_queries_require_financial_access_and_return_patient_identity() {
    let (t, patient) = fixture();
    let view = read(&t, move |conn| {
        patient_billing_view(
            conn,
            &principal(&[Capability::Billing]),
            GetPatientBillingProfileInput {
                subject_id: patient,
                version: None,
            },
        )
    });
    assert_eq!(view.subject.subject_id, patient);
    assert!(view.profile.is_none());
    let denied = t.db.read_blocking(move |conn| {
        patient_billing_view(
            conn,
            &principal(&[Capability::ViewAudit]),
            GetPatientBillingProfileInput {
                subject_id: patient,
                version: None,
            },
        )
    });
    assert_code(denied, ErrorCode::Forbidden);
}

#[test]
fn invalid_financial_input_is_rejected_without_partial_data_or_phi_errors() {
    let (t, patient) = fixture();
    type Change = fn(&mut SetPatientBillingProfileInput);
    let changes: [Change; 5] = [
        |input| {
            input
                .content
                .coverages
                .push(input.content.coverages[0].clone())
        },
        |input| input.content.coverages[0].effective_until = Some("2025-01-01".into()),
        |input| {
            input.content.coverages[0]
                .subscriber
                .as_mut()
                .unwrap()
                .birth_date = Some(Phi::new("CANARY-BAD-DATE".into()))
        },
        |input| input.source = Phi::new(" ".into()),
        |input| input.content.contact.address_lines = vec![Phi::new("x".into()); 4],
    ];
    let before = counts(&t);
    for change in changes {
        let mut candidate = input(patient);
        change(&mut candidate);
        let error = run(&t, &[Capability::Billing], move |ctx| {
            set_patient_billing_profile(ctx, candidate)
        })
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Validation);
        assert!(!format!("{error:?}").contains("CANARY"));
    }
    assert_eq!(counts(&t), before);
}

fn estimate() -> FinancialEvidence {
    FinancialEvidence {
        key: "estimate-request".into(),
        kind: FinancialEvidenceKind::GoodFaithEstimate,
        status: FinancialEvidenceStatus::Pending,
        source_ref: Phi::new("retained-estimate-request".into()),
        scope: Phi::new("Requested synthetic testing".into()),
        occurred_at: test_start(),
        valid_from: None,
        valid_until: None,
        reference_number: None,
        coverage_key: None,
        provider_npi: None,
        units: None,
        notice: None,
        notes: None,
    }
}

#[test]
fn pre_order_estimate_requests_are_retained_but_future_events_and_wrong_coverage_are_refused() {
    let (t, patient) = fixture();
    let mut initial = input(patient);
    initial.content.financial_evidence.push(estimate());
    let saved = run(&t, &[Capability::Billing], move |ctx| {
        set_patient_billing_profile(ctx, initial)
    })
    .unwrap();
    assert_eq!(
        saved.content.financial_evidence[0].status,
        FinancialEvidenceStatus::Pending
    );
    let mut future = input(patient);
    future.expected_version = Some(1);
    let mut evidence = estimate();
    evidence.occurred_at = "2099-01-01T00:00:00.000000000Z".parse().unwrap();
    future.content.financial_evidence.push(evidence);
    assert_code(
        run(&t, &[Capability::Billing], move |ctx| {
            set_patient_billing_profile(ctx, future)
        }),
        ErrorCode::Validation,
    );
    let mut wrong = input(patient);
    wrong.expected_version = Some(1);
    let mut evidence = estimate();
    evidence.coverage_key = Some("absent-plan".into());
    wrong.content.financial_evidence.push(evidence);
    assert_code(
        run(&t, &[Capability::Billing], move |ctx| {
            set_patient_billing_profile(ctx, wrong)
        }),
        ErrorCode::Validation,
    );
}

#[test]
fn profile_versions_are_immutable_and_unknown_input_fields_are_refused() {
    let (t, patient) = fixture();
    run(&t, &[Capability::Billing], move |ctx| {
        set_patient_billing_profile(ctx, input(patient))
    })
    .unwrap();
    for sql in [
        "UPDATE idn_patient_billing_profiles SET reason = 'rewrite'",
        "DELETE FROM idn_patient_billing_profiles",
    ] {
        assert!(
            run(&t, &[Capability::Billing], move |ctx| Ok(ctx
                .tx()
                .execute(sql, [])?))
            .is_err()
        );
    }
    let mut json = serde_json::json!({ "subjectId": patient, "expectedVersion": 1, "content": { "contact": {"addressLines":[],"city":null,"region":null,"postalCode":null,"country":null,"phone":null,"email":null}, "guarantor":null,"coverages":[],"financialEvidence":[]}, "source":"synthetic", "reason":"synthetic" });
    assert!(serde_json::from_value::<SetPatientBillingProfileInput>(json.clone()).is_ok());
    json["content"]["medicalNecessityApproved"] = true.into();
    assert!(serde_json::from_value::<SetPatientBillingProfileInput>(json).is_err());
}
