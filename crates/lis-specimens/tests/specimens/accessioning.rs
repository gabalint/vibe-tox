//! Accessioning and receipt decisions (REQ-SPC-001, REQ-SPC-002, REQ-SPC-008).

use std::collections::BTreeSet;
use std::sync::Arc;

use lis_core::{Capability, ErrorCode};
use lis_orders::{ClarificationDeficiency, PlaceClarificationHoldInput};
use lis_specimens::{
    CollectionObservationInput, CollectionPrecision, CollectionTimeInput, DiscrepancyInput,
    ReceiptDecision, ReceiptDiscrepancyCode, RecordReceiptDecisionInput, SpecimenAcceptance,
    SpecimenInput, VolumeKind,
};

use crate::{
    Fixture, ORAL_FLUID, STAFF, URINE, accession_input, code_of, counts, fixture, last_event, read,
    run,
};

#[test]
fn accession_numbers_containers_and_keeps_collection_evidence() {
    let f = fixture();
    let (order_id, items) = f.accepted_order(&[f.urine_test]);
    let mut input = accession_input(order_id, items.clone());
    input.specimens[0].container_count = 2;
    input.collection.observations = vec![CollectionObservationInput {
        code: "temperature_in_range".to_owned(),
        value_text: "yes".to_owned(),
        observed_by: "Synthetic Collector".to_owned(),
    }];
    let detail = run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    })
    .unwrap();
    // The test clock is in 2026; the defaults are A + yy + 6 digits and C + 8 digits.
    assert_eq!(detail.accession_number, "A26000001");
    assert_eq!(
        (detail.order_id, detail.order_item_ids.clone()),
        (order_id, items)
    );
    assert_eq!(detail.subject_id, f.subject_id);
    let collection = &detail.collection;
    assert_eq!(collection.precision, CollectionPrecision::Datetime);
    assert_eq!(
        collection.collected_at.unwrap().to_string(),
        "2026-01-01T14:30:00.000000000Z"
    );
    assert_eq!(collection.utc_offset_minutes, Some(-300));
    assert_eq!(
        collection.observations[0].observed_by,
        "Synthetic Collector"
    );
    let specimen = &detail.specimens[0];
    assert_eq!(specimen.acceptance, SpecimenAcceptance::Received);
    assert_eq!(specimen.allowed_actions, ["record_receipt_decision"]);
    let codes: Vec<&str> = specimen
        .containers
        .iter()
        .map(|container| container.label_code.as_str())
        .collect();
    assert_eq!(codes, ["C00000001", "C00000002"]);
    assert_eq!(
        specimen.containers[0].volume.as_ref().unwrap().as_str(),
        "10.0"
    );
    let (action, record, _) = last_event(&f.t);
    assert_eq!(action, "specimen.create_accession");
    assert_eq!(record, Some(detail.accession_id.to_string()));
}

#[test]
fn unknown_and_date_only_collection_times_stay_explicit() {
    let f = fixture();
    for (time, precision) in [
        (CollectionTimeInput::Unknown, CollectionPrecision::Unknown),
        (
            CollectionTimeInput::Date {
                date: "2026-01-01".to_owned(),
            },
            CollectionPrecision::Date,
        ),
    ] {
        let (order_id, items) = f.accepted_order(&[f.urine_test]);
        let mut input = accession_input(order_id, items);
        input.collection.time = time;
        let detail = run(&f.t, STAFF, move |ctx| {
            lis_specimens::create_accession(ctx, input)
        })
        .unwrap();
        assert_eq!(detail.collection.precision, precision);
        assert_eq!(detail.collection.collected_at, None);
    }
}

#[test]
fn numbering_is_unique_under_the_single_writer() {
    let f = Arc::new(fixture());
    #[expect(
        clippy::needless_collect,
        reason = "every writer thread starts before any is joined"
    )]
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let (order_id, items) = f.accepted_order(&[f.urine_test]);
            let f = Arc::clone(&f);
            std::thread::spawn(move || {
                let mut input = accession_input(order_id, items);
                input.specimens[0].container_count = 2;
                run(&f.t, STAFF, move |ctx| {
                    lis_specimens::create_accession(ctx, input)
                })
                .unwrap()
            })
        })
        .collect();
    let details: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let accessions: BTreeSet<String> = details
        .iter()
        .map(|detail| detail.accession_number.clone())
        .collect();
    let containers: BTreeSet<String> = details
        .iter()
        .flat_map(|detail| &detail.specimens[0].containers)
        .map(|container| container.label_code.clone())
        .collect();
    let expected: BTreeSet<String> = (1..=6).map(|n| format!("A26{n:06}")).collect();
    assert_eq!(accessions, expected);
    let expected: BTreeSet<String> = (1..=12).map(|n| format!("C{n:08}")).collect();
    assert_eq!(containers, expected);
}

/// The accession is refused with `code` and nothing is written.
fn refused(f: &Fixture, input: lis_specimens::CreateAccessionInput, code: ErrorCode) {
    let before = counts(&f.t);
    let result = run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    });
    assert_eq!(code_of(result), code);
    assert_eq!(counts(&f.t), before);
}

#[test]
fn items_of_an_order_not_yet_accepted_are_refused() {
    let f = fixture();
    let order = f.received_order(&[f.urine_test]);
    let items = order.items.iter().map(|item| item.item_id).collect();
    refused(
        &f,
        accession_input(order.order_id, items),
        ErrorCode::GateBlocked,
    );
}

#[test]
fn items_of_an_order_with_an_open_clarification_hold_are_refused() {
    let f = fixture();
    let (order_id, items) = f.accepted_order(&[f.urine_test]);
    let revision = read(&f.t, move |conn| {
        Ok(lis_orders::queries::get_order(conn, order_id)
            .unwrap()
            .revision)
    });
    run(&f.t, &[Capability::EnterOrders], move |ctx| {
        lis_orders::place_clarification_hold(
            ctx,
            PlaceClarificationHoldInput {
                order_id,
                expected_revision: revision,
                deficiency: ClarificationDeficiency::SignatureOrIntentRequired,
                note: None,
            },
        )
        .map_err(lis_specimens::SpecimenError::from)
    })
    .unwrap();
    refused(&f, accession_input(order_id, items), ErrorCode::GateBlocked);
}

#[test]
fn an_item_is_accessioned_once() {
    let f = fixture();
    let (order_id, items) = f.accepted_order(&[f.urine_test]);
    let input = accession_input(order_id, items.clone());
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    })
    .unwrap();
    refused(&f, accession_input(order_id, items), ErrorCode::Conflict);
}

#[test]
fn specimens_must_match_the_matrices_the_items_require() {
    let f = fixture();
    let (order_id, items) = f.accepted_order(&[f.urine_test, f.oral_fluid_test]);
    // Missing the oral fluid specimen.
    refused(
        &f,
        accession_input(order_id, items.clone()),
        ErrorCode::Validation,
    );
    // A matrix no selected item requires.
    let mut wrong = accession_input(order_id, vec![items[0]]);
    wrong.specimens.push(SpecimenInput {
        specimen_type_code: ORAL_FLUID.to_owned(),
        container_count: 1,
        volume: None,
        volume_unit: None,
        volume_kind: VolumeKind::Unknown,
    });
    refused(&f, wrong, ErrorCode::Validation);
    let mut both = accession_input(order_id, items);
    both.specimens.push(SpecimenInput {
        specimen_type_code: ORAL_FLUID.to_owned(),
        container_count: 1,
        volume: None,
        volume_unit: None,
        volume_kind: VolumeKind::Unknown,
    });
    let detail = run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, both)
    })
    .unwrap();
    let matrices: BTreeSet<&str> = detail
        .specimens
        .iter()
        .map(|specimen| specimen.specimen_type_code.as_str())
        .collect();
    assert_eq!(matrices, BTreeSet::from([URINE, ORAL_FLUID]));
}

#[test]
fn malformed_accession_input_is_refused() {
    let f = fixture();
    let (order_id, items) = f.accepted_order(&[f.urine_test]);
    let mut future = accession_input(order_id, items.clone());
    future.collection.time = CollectionTimeInput::Datetime {
        local: "2027-01-01T09:30".to_owned(),
        utc_offset_minutes: 0,
    };
    refused(&f, future, ErrorCode::Validation);
    let mut no_containers = accession_input(order_id, items.clone());
    no_containers.specimens[0].container_count = 0;
    refused(&f, no_containers, ErrorCode::Validation);
    let mut duplicate = accession_input(order_id, items.clone());
    duplicate.order_item_ids.push(items[0]);
    refused(&f, duplicate, ErrorCode::Validation);
    let mut free_text_code = accession_input(order_id, items);
    free_text_code.collection.observations = vec![CollectionObservationInput {
        code: "Temperature OK?".to_owned(),
        value_text: "yes".to_owned(),
        observed_by: "Synthetic Collector".to_owned(),
    }];
    refused(&f, free_text_code, ErrorCode::Validation);
}

fn decision(
    specimen: lis_specimens::SpecimenId,
    expected_revision: i64,
    decision: ReceiptDecision,
) -> RecordReceiptDecisionInput {
    RecordReceiptDecisionInput {
        specimen_id: specimen,
        expected_revision,
        decision,
        discrepancies: Vec::new(),
        policy_reference: None,
        scope: None,
        reason: None,
    }
}

fn decide(
    f: &Fixture,
    input: RecordReceiptDecisionInput,
) -> Result<lis_specimens::AccessionDetail, lis_specimens::SpecimenError> {
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(ctx, input)
    })
}

#[test]
fn conditional_acceptance_needs_policy_reference_and_scope() {
    let f = fixture();
    let specimen = f.accession(1).specimens[0].specimen_id;
    let bare = decision(specimen, 1, ReceiptDecision::AcceptedConditionally);
    assert_eq!(code_of(decide(&f, bare.clone())), ErrorCode::Validation);
    let mut policy_only = bare;
    policy_only.policy_reference = Some("SYN-SOP-12 §4".to_owned());
    assert_eq!(
        code_of(decide(&f, policy_only.clone())),
        ErrorCode::Validation
    );
    let mut complete = policy_only;
    complete.scope = Some("Volume below minimum; screen only".to_owned());
    complete.discrepancies = vec![DiscrepancyInput {
        code: ReceiptDiscrepancyCode::InsufficientVolume,
        note: None,
    }];
    let detail = decide(&f, complete).unwrap();
    let specimen = &detail.specimens[0];
    assert_eq!(
        specimen.acceptance,
        SpecimenAcceptance::AcceptedConditionally
    );
    assert_eq!(specimen.revision, 2);
    let recorded = &specimen.decisions[0];
    assert_eq!(recorded.policy_reference.as_deref(), Some("SYN-SOP-12 §4"));
    assert_eq!(
        recorded.discrepancies[0].code,
        ReceiptDiscrepancyCode::InsufficientVolume
    );
    assert!(specimen.allowed_actions.is_empty());
}

#[test]
fn hold_and_rejection_need_a_reason_and_follow_the_state_machine() {
    let f = fixture();
    let specimen = f.accession(1).specimens[0].specimen_id;
    let hold = decision(specimen, 1, ReceiptDecision::Held);
    assert_eq!(code_of(decide(&f, hold.clone())), ErrorCode::Validation);
    let mut hold = hold;
    hold.reason = Some("Label unreadable; calling the clinic".to_owned());
    hold.discrepancies = vec![DiscrepancyInput {
        code: ReceiptDiscrepancyCode::Other,
        note: None,
    }];
    // `other` needs a note.
    assert_eq!(code_of(decide(&f, hold.clone())), ErrorCode::Validation);
    hold.discrepancies[0].note = Some("Smudged label".to_owned());
    decide(&f, hold.clone()).unwrap();
    let (action, _, reason) = last_event(&f.t);
    assert_eq!(action, "specimen.record_receipt_decision");
    assert_eq!(
        reason.as_deref(),
        Some("Label unreadable; calling the clinic")
    );
    // Held twice is refused; a stale revision is a conflict.
    let mut again = hold;
    again.expected_revision = 2;
    assert_eq!(code_of(decide(&f, again)), ErrorCode::Conflict);
    let stale = decision(specimen, 1, ReceiptDecision::Accepted);
    assert_eq!(code_of(decide(&f, stale)), ErrorCode::Conflict);
    decide(&f, decision(specimen, 2, ReceiptDecision::Accepted)).unwrap();
    // Accepted is final in M2.
    let mut reject = decision(specimen, 3, ReceiptDecision::Rejected);
    reject.reason = Some("Synthetic".to_owned());
    assert_eq!(code_of(decide(&f, reject)), ErrorCode::Conflict);
}

#[test]
fn policy_fields_belong_only_to_conditional_acceptance() {
    let f = fixture();
    let specimen = f.accession(1).specimens[0].specimen_id;
    let mut accepted = decision(specimen, 1, ReceiptDecision::Accepted);
    accepted.scope = Some("Synthetic scope".to_owned());
    assert_eq!(code_of(decide(&f, accepted)), ErrorCode::Validation);
}

#[test]
fn receipt_history_uses_committed_order_after_clock_rollback() {
    let f = fixture();
    let specimen = f.accession(1).specimens[0].specimen_id;
    let mut held = decision(specimen, 1, ReceiptDecision::Held);
    held.reason = Some("Synthetic receipt investigation".to_owned());
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(ctx, held)
    })
    .unwrap();
    f.t.clock
        .set(lis_core::UtcTimestamp::parse("2020-01-01T00:00:00.000000000Z").unwrap());
    let accepted = decision(specimen, 2, ReceiptDecision::Accepted);
    let after = run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(ctx, accepted)
    })
    .unwrap();
    let history = &after.specimens[0].decisions;
    assert_eq!(history[0].decision, ReceiptDecision::Held);
    assert_eq!(history[1].decision, ReceiptDecision::Accepted);
    assert!(history[1].decided_at < history[0].decided_at);
    assert_eq!(history[1].causal_sequence, Some(3));
}

#[test]
fn legacy_receipt_resolution_requires_quality_current_revision_and_audits_evidence() {
    let f = fixture();
    let specimen_id = f.accession(1).specimens[0].specimen_id;
    let mut held = decision(specimen_id, 1, ReceiptDecision::Held);
    held.reason = Some("Synthetic hold".to_owned());
    decide(&f, held).unwrap();
    let detail = decide(&f, decision(specimen_id, 2, ReceiptDecision::Accepted)).unwrap();
    let ids: Vec<_> = detail.specimens[0]
        .decisions
        .iter()
        .map(|d| d.decision_id)
        .collect();
    // Simulate a pre-migration record whose audit link cannot be established.
    let conn = crate::raw(&f);
    conn.execute_batch("DROP TRIGGER spc_receipt_order_no_delete")
        .unwrap();
    conn.execute(
        "DELETE FROM spc_receipt_order WHERE decision_id = ?1",
        [ids[0]],
    )
    .unwrap();
    drop(conn);
    let input = lis_specimens::ResolveReceiptChronologyInput {
        specimen_id,
        expected_revision: 3,
        decision_ids: ids,
        reason: "Synthetic retained receipt ledger confirms hold then acceptance".to_owned(),
    };
    let denied = input.clone();
    assert_eq!(
        code_of(run(&f.t, &[lis_core::Capability::Accession], move |ctx| {
            lis_specimens::resolve_receipt_chronology(ctx, denied)
        })),
        ErrorCode::Forbidden
    );
    let mut stale = input.clone();
    stale.expected_revision = 2;
    assert_eq!(
        code_of(run(&f.t, STAFF, move |ctx| {
            lis_specimens::resolve_receipt_chronology(ctx, stale)
        })),
        ErrorCode::Conflict
    );
    let mut reversed = input.clone();
    reversed.decision_ids.reverse();
    assert!(
        run(&f.t, STAFF, move |ctx| {
            lis_specimens::resolve_receipt_chronology(ctx, reversed)
        })
        .is_err()
    );
    let detail = run(&f.t, STAFF, move |ctx| {
        lis_specimens::resolve_receipt_chronology(ctx, input)
    })
    .unwrap();
    assert!(
        detail.specimens[0]
            .decisions
            .iter()
            .all(|d| d.causal_sequence.is_some())
    );
    assert_eq!(detail.specimens[0].revision, 4);
    assert_eq!(last_event(&f.t).0, "specimen.resolve_receipt_chronology");
    assert!(last_event(&f.t).2.unwrap().contains("ledger"));
}
