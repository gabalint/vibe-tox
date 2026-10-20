//! REQ-ORD-001/002, REQ-SPC-001/002: actionable remaining items and receipt progress.

use lis_orders::{ClarificationDeficiency, OrderId, PlaceClarificationHoldInput};
use lis_specimens::{OrderAccessionContext, ReceiptDecision, RecordReceiptDecisionInput};

use crate::{Fixture, ORAL_FLUID, STAFF, URINE, accession_input, counts, fixture, read, run};

fn context(f: &Fixture, order_id: OrderId) -> OrderAccessionContext {
    let before = counts(&f.t);
    let context = read(&f.t, move |conn| {
        lis_specimens::get_order_accession_context(conn, order_id)
    });
    assert_eq!(counts(&f.t), before, "domain read writes no state or audit");
    context
}

#[test]
fn unaccepted_order_items_have_no_accession_action_or_fabricated_matrices() {
    let f = fixture();
    let order = f.received_order(&[f.urine_test]);
    let view = context(&f, order.order_id);
    assert_eq!(view.items.len(), 1);
    assert!(!view.items[0].eligible);
    assert_eq!(
        view.items[0].reason.as_deref(),
        Some("The order has not been accepted.")
    );
    assert!(view.items[0].required_specimen_type_codes.is_empty());
    assert!(view.accessions.is_empty());
    assert!(view.allowed_actions.is_empty());
}

#[test]
fn accepted_items_are_eligible_until_a_clarification_hold_is_opened() {
    let f = fixture();
    let (order_id, _) = f.accepted_order(&[f.urine_test]);
    let ready = context(&f, order_id);
    assert!(ready.items[0].eligible);
    assert!(ready.items[0].reason.is_none());
    assert_eq!(ready.items[0].required_specimen_type_codes, [URINE]);
    assert_eq!(ready.allowed_actions, ["create_accession"]);
    let revision = read(&f.t, move |conn| {
        Ok(lis_orders::get_order(conn, order_id)?.revision)
    });
    run(&f.t, STAFF, move |ctx| {
        Ok(lis_orders::place_clarification_hold(
            ctx,
            PlaceClarificationHoldInput {
                order_id,
                expected_revision: revision,
                deficiency: ClarificationDeficiency::OtherAuthority,
                note: None,
            },
        )?)
    })
    .unwrap();
    let held = context(&f, order_id);
    assert!(!held.items[0].eligible);
    assert_eq!(
        held.items[0].reason.as_deref(),
        Some("An order clarification hold is open.")
    );
    assert!(held.allowed_actions.is_empty());
}

#[test]
fn partial_accession_keeps_remaining_items_and_counts_specimens_instead_of_containers() {
    let f = fixture();
    let (order_id, _) = f.accepted_order(&[f.urine_test, f.oral_fluid_test]);
    let before = context(&f, order_id);
    let urine = before
        .items
        .iter()
        .find(|item| item.required_specimen_type_codes == [URINE])
        .unwrap()
        .order_item_id;
    let oral = before
        .items
        .iter()
        .find(|item| item.required_specimen_type_codes == [ORAL_FLUID])
        .unwrap()
        .order_item_id;
    let mut input = accession_input(order_id, vec![urine]);
    input.specimens[0].container_count = 2;
    let accession = run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    })
    .unwrap();
    let partial = context(&f, order_id);
    let linked = partial
        .items
        .iter()
        .find(|item| item.order_item_id == urine)
        .unwrap();
    assert!(!linked.eligible);
    assert_eq!(linked.accession_id, Some(accession.accession_id));
    assert_eq!(
        linked.accession_number.as_deref(),
        Some(accession.accession_number.as_str())
    );
    assert_eq!(
        linked.reason.as_deref(),
        Some("The ordered test already has an accession.")
    );
    assert!(
        partial
            .items
            .iter()
            .find(|item| item.order_item_id == oral)
            .unwrap()
            .eligible
    );
    assert_eq!(partial.allowed_actions, ["create_accession"]);
    assert_eq!(partial.accessions.len(), 1);
    assert_eq!(partial.accessions[0].specimen_count, 1);
    assert_eq!(partial.accessions[0].pending_receipt_count, 1);
    let specimen = &accession.specimens[0];
    let accept = RecordReceiptDecisionInput {
        specimen_id: specimen.specimen_id,
        expected_revision: specimen.revision,
        decision: ReceiptDecision::Accepted,
        discrepancies: Vec::new(),
        policy_reference: None,
        scope: None,
        reason: None,
    };
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(ctx, accept)
    })
    .unwrap();
    assert_eq!(context(&f, order_id).accessions[0].pending_receipt_count, 0);
    let mut input = accession_input(order_id, vec![oral]);
    input.specimens[0].specimen_type_code = ORAL_FLUID.to_owned();
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    })
    .unwrap();
    let complete = context(&f, order_id);
    assert_eq!(complete.accessions.len(), 2);
    assert!(complete.items.iter().all(|item| !item.eligible));
    assert!(complete.allowed_actions.is_empty());
}

#[test]
fn unknown_order_fails_instead_of_showing_an_empty_context() {
    let f = fixture();
    let result = f.t.db.read_blocking(|conn| {
        lis_specimens::get_order_accession_context(conn, OrderId::generate())
    });
    assert_eq!(result.unwrap_err().code(), lis_core::ErrorCode::NotFound);
}

#[test]
fn cancelled_items_keep_their_accession_link_but_cannot_create_another() {
    let f = fixture();
    let accession = f.accession(1);
    let item_id = accession.order_item_ids[0];
    let order = run(&f.t, STAFF, move |ctx| {
        Ok(lis_orders::request_cancellation(
            ctx,
            lis_orders::RequestCancellationInput {
                item_id,
                requested_by: "Synthetic clinician".to_owned(),
                reason: "Synthetic cancellation".to_owned(),
            },
        )?)
    })
    .unwrap();
    let request = &order.cancellations[0];
    let decision = lis_orders::DecideCancellationInput {
        request_id: request.request_id,
        expected_revision: request.revision,
        decision: lis_orders::Decision::Accept,
        reason: "Synthetic cancellation accepted".to_owned(),
    };
    run(&f.t, STAFF, move |ctx| {
        Ok(lis_orders::decide_cancellation(ctx, decision)?)
    })
    .unwrap();
    let view = context(&f, accession.order_id);
    assert!(!view.items[0].eligible);
    assert_eq!(
        view.items[0].reason.as_deref(),
        Some("The ordered test was cancelled.")
    );
    assert_eq!(view.items[0].accession_id, Some(accession.accession_id));
    assert!(view.allowed_actions.is_empty());
}
