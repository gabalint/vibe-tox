//! DM-06 uses synthetic source evidence; no inferred patient demographics.
#![cfg(test)]
use lis_core::{Capability, CorrelationId, ErrorCode, Phi};
use lis_identity::*;
use lis_store::testing::{TestDatabase, principal};

fn input(kind: BirthEvidenceKind, text: Option<&str>) -> RecordIntakeDeferralInput {
    RecordIntakeDeferralInput {
        supplied_name: Phi::new("Synthetic Intake".into()),
        birth_evidence_kind: kind,
        birth_evidence_text: text.map(|s| Phi::new(s.into())),
        source_reference: "synthetic-requisition-A".into(),
        reason: "Complete birth date not supplied".into(),
    }
}
fn record(t: &TestDatabase, value: RecordIntakeDeferralInput) -> IntakeDeferralView {
    t.db.write_blocking(
        principal(&[Capability::EnterOrders]),
        CorrelationId::generate(),
        move |ctx| record_intake_deferral(ctx, value),
    )
    .unwrap()
}
#[test]
fn intake_retains_unknown_partial_and_age_without_creating_subjects() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    for (kind, text) in [
        (BirthEvidenceKind::Unknown, None),
        (BirthEvidenceKind::PartialDate, Some("1980-??-??")),
        (
            BirthEvidenceKind::SuppliedAge,
            Some("approximately 45 years as supplied"),
        ),
    ] {
        let saved = record(&t, input(kind, text));
        assert_eq!(
            saved
                .birth_evidence_text
                .as_ref()
                .map(|v| v.expose().as_str()),
            text
        );
        assert_eq!(saved.revision, 1);
        assert!(saved.subject_id.is_none());
    }
    let n: i64 =
        t.db.read_blocking(|conn| -> Result<i64, IdentityError> {
            Ok(conn.query_row("SELECT count(*) FROM idn_subjects", [], |r| r.get(0))?)
        })
        .unwrap();
    assert_eq!(n, 0);
    let denied =
        t.db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
            record_intake_deferral(ctx, input(BirthEvidenceKind::Unknown, None))
        })
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
}
#[test]
fn intake_resolution_is_attributable_terminal_and_never_erases_original() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let saved = record(&t, input(BirthEvidenceKind::SuppliedAge, Some("age 42")));
    let id = saved.intake_id;
    let resolution = ResolveIntakeDeferralInput {
        intake_id: id,
        expected_revision: 1,
        disposition: IntakeDisposition::Declined,
        subject_id: None,
        source_reference: "synthetic-contact-record".into(),
        reason: "Source could not establish complete identity; intake declined".into(),
    };
    let retry = resolution.clone();
    let done =
        t.db.write_blocking(
            principal(&[Capability::EnterOrders]),
            CorrelationId::generate(),
            move |ctx| resolve_intake_deferral(ctx, resolution),
        )
        .unwrap();
    assert_eq!(done.revision, 2);
    assert_eq!(done.birth_evidence_text.unwrap().expose(), "age 42");
    assert!(done.disposed_by.is_some());
    let stale =
        t.db.write_blocking(
            principal(&[Capability::EnterOrders]),
            CorrelationId::generate(),
            move |ctx| resolve_intake_deferral(ctx, retry),
        )
        .unwrap_err();
    assert_eq!(stale.code(), ErrorCode::Conflict);
    let open =
        t.db.read_blocking(|conn| {
            list_intake_deferrals(
                conn,
                ListIntakeDeferralsInput {
                    include_closed: false,
                    after_id: None,
                },
            )
        })
        .unwrap();
    assert!(open.is_empty());
    t.db.read_blocking(move |conn| -> Result<(), IdentityError> {
        assert!(
            conn.execute(
                "UPDATE idn_intake_deferrals SET supplied_name='changed' WHERE id=?1",
                [id]
            )
            .is_err()
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn intake_pages_reach_all_records() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    for _ in 0..51 {
        record(&t, input(BirthEvidenceKind::Unknown, None));
    }
    let page =
        t.db.read_blocking(|conn| {
            list_intake_deferrals(
                conn,
                ListIntakeDeferralsInput {
                    include_closed: false,
                    after_id: None,
                },
            )
        })
        .unwrap();
    assert_eq!(page.len(), 50);
    let after = page.last().unwrap().intake_id;
    let rest =
        t.db.read_blocking(move |conn| {
            list_intake_deferrals(
                conn,
                ListIntakeDeferralsInput {
                    include_closed: false,
                    after_id: Some(after),
                },
            )
        })
        .unwrap();
    assert_eq!(rest.len(), 1);
    assert!(!page.iter().any(|row| row.intake_id == rest[0].intake_id));
}

#[test]
fn intake_resolves_only_to_an_existing_patient() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let id = record(&t, input(BirthEvidenceKind::Unknown, None)).intake_id;
    let invalid = ResolveIntakeDeferralInput {
        intake_id: id,
        expected_revision: 1,
        disposition: IntakeDisposition::Resolved,
        subject_id: Some(SubjectId::generate()),
        source_reference: "new supplied evidence".into(),
        reason: "verified identity".into(),
    };
    assert_eq!(
        t.db.write_blocking(
            principal(&[Capability::EnterOrders]),
            CorrelationId::generate(),
            move |ctx| resolve_intake_deferral(ctx, invalid)
        )
        .unwrap_err()
        .code(),
        ErrorCode::NotFound
    );
    let subject =
        t.db.write_blocking(
            principal(&[Capability::EnterOrders]),
            CorrelationId::generate(),
            |ctx| {
                register_subject(
                    ctx,
                    RegisterSubject {
                        demographics: DemographicsInput {
                            family_name: Phi::new("Synthetic Intake".into()),
                            given_names: None,
                            birth_date: Phi::new("1984-02-03".into()),
                            administrative_sex: None,
                        },
                        source: "new source-backed full date".into(),
                        identifiers: vec![],
                        acknowledged_candidates: vec![],
                    },
                )
            },
        )
        .unwrap();
    let resolved =
        t.db.write_blocking(
            principal(&[Capability::EnterOrders]),
            CorrelationId::generate(),
            move |ctx| {
                resolve_intake_deferral(
                    ctx,
                    ResolveIntakeDeferralInput {
                        intake_id: id,
                        expected_revision: 1,
                        disposition: IntakeDisposition::Resolved,
                        subject_id: Some(subject.subject_id),
                        source_reference: "synthetic-verified-identity".into(),
                        reason: "Verified source confirms this registered patient".into(),
                    },
                )
            },
        )
        .unwrap();
    assert!(resolved.subject_id.is_some());
    assert_eq!(resolved.birth_evidence_kind, BirthEvidenceKind::Unknown);
}

#[test]
fn intake_disposition_denial_is_atomic_and_audit_records_actual_authority() {
    let t = TestDatabase::open(&[SCHEMA]).unwrap();
    let saved = record(&t, input(BirthEvidenceKind::Unknown, None));
    let id = saved.intake_id;
    let value = ResolveIntakeDeferralInput {
        intake_id: id,
        expected_revision: 1,
        disposition: IntakeDisposition::Declined,
        subject_id: None,
        source_reference: "synthetic-refusal".into(),
        reason: "Source declined identity clarification".into(),
    };
    let retry = value.clone();
    assert_eq!(
        t.db.write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
            resolve_intake_deferral(ctx, value)
        })
        .unwrap_err()
        .code(),
        ErrorCode::Forbidden
    );
    let open =
        t.db.read_blocking(|conn| {
            list_intake_deferrals(
                conn,
                ListIntakeDeferralsInput {
                    include_closed: false,
                    after_id: None,
                },
            )
        })
        .unwrap();
    assert_eq!(open.len(), 1);
    let actor = principal(&[Capability::EnterOrders]);
    let who = actor.actor().id_text();
    let correlation = CorrelationId::generate();
    t.db.write_blocking(actor, correlation, move |ctx| {
        resolve_intake_deferral(ctx, retry)
    })
    .unwrap();
    t.db.read_blocking(move |conn| -> Result<(), IdentityError> {
        let (authority, record, corr): (String, String, String) = conn.query_row(
            "SELECT authority, record_id, correlation_id FROM audit_events \
             WHERE action='identity.resolve_intake_deferral'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(authority, "enter_orders");
        assert_eq!(record, id.to_string());
        assert_eq!(corr, correlation.to_string());
        let author: String = conn.query_row(
            "SELECT recorded_by FROM idn_intake_dispositions WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        assert_eq!(author, who);
        Ok(())
    })
    .unwrap();
}
