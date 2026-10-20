//! DM-02: collection evidence, not delayed intake or review, controls standing authority.
use crate::{Fixture, STAFF, accession_input, fixture, read, run, setup};
use lis_orders::{AcceptOrderInput, RegisterStandingOrderInput};
use lis_specimens::{CollectionTimeInput, ReceiptDecision, RecordReceiptDecisionInput};

fn standing(f: &Fixture, limit: Option<u32>) -> lis_orders::StandingOrderId {
    let version = f.urine_test;
    let scope =
        f.t.db
            .read_blocking(move |conn| lis_catalog::get_orderable_version(conn, version))
            .unwrap()
            .orderable_id;
    let (subject_id, practitioner_id) = (f.subject_id, f.practitioner_id);
    setup(&f.t, move |ctx| {
        lis_orders::register_standing_order(
            ctx,
            RegisterStandingOrderInput {
                subject_id,
                practitioner_id,
                scope_orderable_ids: vec![scope],
                valid_from: "2025-01-01".into(),
                valid_until: "2025-12-31".into(),
                visit_limit: limit,
                document_ref: "SYN-PATIENT-STANDING".into(),
            },
        )
    })
    .standing_order_id
}
fn accepted(
    f: &Fixture,
    standing: lis_orders::StandingOrderId,
    time: CollectionTimeInput,
) -> lis_specimens::AccessionDetail {
    let order = f.received_standing_order(&[f.urine_test], Some(standing));
    let order = setup(&f.t, move |ctx| {
        lis_orders::accept_order(
            ctx,
            AcceptOrderInput {
                order_id: order.order_id,
                expected_revision: order.revision,
            },
        )
    });
    assert_eq!(order.status, lis_orders::OrderStatus::Accepted);
    let mut input = accession_input(
        order.order_id,
        order.items.iter().map(|item| item.item_id).collect(),
    );
    input.collection.time = time;
    let accession = run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    })
    .unwrap();
    let specimen = accession.specimens[0].specimen_id;
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(
            ctx,
            RecordReceiptDecisionInput {
                specimen_id: specimen,
                expected_revision: 1,
                decision: ReceiptDecision::Accepted,
                discrepancies: vec![],
                policy_reference: None,
                scope: None,
                reason: None,
            },
        )
    })
    .unwrap()
}
fn codes(f: &Fixture, id: lis_specimens::SpecimenId) -> Vec<String> {
    let gates = read(&f.t, move |conn| lis_specimens::release_gate(conn, id));
    gates
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|gate| gate.code)
        .collect()
}
#[test]
fn delayed_collection_uses_original_civil_date_and_reserves_exactly_one_use() {
    let f = fixture();
    let authority = standing(&f, Some(1));
    // UTC is January 1, but the supplied source offset proves collection on December 31.
    let accession = accepted(
        &f,
        authority,
        CollectionTimeInput::Datetime {
            local: "2025-12-31T23:30".into(),
            utc_offset_minutes: -300,
        },
    );
    let specimen = accession.specimens[0].specimen_id;
    assert!(codes(&f, specimen).is_empty());
    let container = accession.specimens[0].containers[0].container_id;
    assert!(
        read(&f.t, move |conn| lis_specimens::testing_gate(
            conn,
            container,
            crate::URINE
        ))
        .is_ok()
    );
    let second = f.received_standing_order(&[f.urine_test], Some(authority));
    let second = setup(&f.t, move |ctx| {
        lis_orders::accept_order(
            ctx,
            AcceptOrderInput {
                order_id: second.order_id,
                expected_revision: second.revision,
            },
        )
    });
    assert_eq!(second.status, lis_orders::OrderStatus::Received);
    assert!(!second.holds.is_empty());
    assert!(
        codes(&f, specimen).is_empty(),
        "Exhausted remaining uses cannot invalidate the already reserved visit"
    );
}
#[test]
fn unknown_and_corrected_outside_dates_remain_blocked_after_quality_review() {
    let f = fixture();
    let authority = standing(&f, None);
    let unknown = accepted(&f, authority, CollectionTimeInput::Unknown);
    assert!(
        codes(&f, unknown.specimens[0].specimen_id).contains(&"standing_collection_unknown".into())
    );
    let accession = accepted(
        &f,
        authority,
        CollectionTimeInput::Date {
            date: "2025-12-31".into(),
        },
    );
    let accession_id = accession.accession_id;
    let specimen = accession.specimens[0].specimen_id;
    let correction = run(&f.t, STAFF, move |ctx| {
        lis_specimens::correct_collection_time(
            ctx,
            lis_specimens::CorrectCollectionTimeInput {
                accession_id,
                expected_revision: 1,
                time: CollectionTimeInput::Date {
                    date: "2026-01-01".into(),
                },
                source_document_ref: "SYN-CORRECTED-DATE".into(),
                reason: "Supplied correction".into(),
            },
        )
    })
    .unwrap();
    let correction_id = correction.corrections[0].correction_id;
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::resolve_collection_correction(
            ctx,
            lis_specimens::ResolveCollectionCorrectionInput {
                correction_id,
                reason: "Stability reviewed; ordering authorization not extended".into(),
            },
        )
    })
    .unwrap();
    let gates = codes(&f, specimen);
    assert!(!gates.contains(&"collection_time_review".into()));
    assert!(gates.contains(&"standing_collection_outside_validity".into()));
    let container = accession.specimens[0].containers[0].container_id;
    let testing = read(&f.t, move |conn| {
        lis_specimens::testing_gate(conn, container, crate::URINE)
    })
    .unwrap_err();
    assert!(
        testing
            .iter()
            .any(|gate| gate.code == "standing_collection_outside_validity")
    );
}
