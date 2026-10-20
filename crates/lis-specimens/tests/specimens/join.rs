//! Joining an accepted add-on item to the accession whose material was
//! measured (result files brief F17, §13; REQ-SPC-001).

use lis_core::{Capability, ErrorCode};
use lis_orders::{
    DecideFollowOnInput, Decision, ItemKind, OrderId, OrderItemId, RequestFollowOnInput,
    SequenceRole,
};
use lis_specimens::{AccessionId, SpecimenError};

use crate::{Fixture, STAFF, accession_input, counts, fixture, read, run};

/// Requests `test` as an add-on of `order_id`, accepted when `accept` is set.
fn add_on(
    f: &Fixture,
    order_id: OrderId,
    test: lis_catalog::OrderableVersionId,
    accept: bool,
) -> OrderItemId {
    let revision = read(&f.t, move |conn| {
        Ok(lis_orders::get_order(conn, order_id)?.revision)
    });
    let request = RequestFollowOnInput {
        order_id,
        expected_revision: revision,
        kind: ItemKind::AddOn,
        orderable_version_id: test,
        sequence_role: SequenceRole::FirstLine,
        parent_item_id: None,
        authority_reference: Some("SYN-ADDON-1".to_owned()),
        poc_result_id: None,
        trigger_source: None,
        trigger_ref: None,
        reason: "Synthetic clinician call".to_owned(),
    };
    let order = run(&f.t, &[Capability::EnterOrders], move |ctx| {
        Ok(lis_orders::request_follow_on(ctx, request)?)
    })
    .unwrap();
    let item = order
        .items
        .iter()
        .rfind(|item| item.kind == ItemKind::AddOn)
        .unwrap();
    let item_id = item.item_id;
    if accept {
        let decide = DecideFollowOnInput {
            item_id,
            expected_revision: item.revision,
            decision: Decision::Accept,
            reason: "Synthetic acceptance".to_owned(),
        };
        run(&f.t, &[Capability::EnterOrders], move |ctx| {
            Ok(lis_orders::decide_follow_on(ctx, decide)?)
        })
        .unwrap();
    }
    item_id
}

/// An accession of an accepted urine order; returns the order and accession.
fn accessioned(f: &Fixture) -> (OrderId, AccessionId) {
    let (order_id, items) = f.accepted_order(&[f.urine_test]);
    let input = accession_input(order_id, items);
    let accession = run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_accession(ctx, input)
    })
    .unwrap();
    (order_id, accession.accession_id)
}

/// `join_accession` with no capability of its own: the caller checks it.
fn join(f: &Fixture, accession: AccessionId, item: OrderItemId) -> Result<(), SpecimenError> {
    run(&f.t, &[], move |ctx| {
        lis_specimens::join_accession(ctx, accession, item)
    })
}

fn items(f: &Fixture, accession: AccessionId) -> Vec<OrderItemId> {
    read(&f.t, move |conn| {
        Ok(lis_specimens::get_accession(conn, accession)?.order_item_ids)
    })
}

#[test]
fn an_accepted_add_on_joins_the_accession_of_its_order_without_an_event() {
    let f = fixture();
    let (order_id, accession) = accessioned(&f);
    let add_on = add_on(&f, order_id, f.urine_test, true);
    let before = counts(&f.t);
    join(&f, accession, add_on).unwrap();
    assert!(items(&f, accession).contains(&add_on));
    assert!(read(&f.t, move |conn| {
        lis_specimens::order_item_accessioned(conn, add_on)
    }));
    let after = counts(&f.t);
    let changed = before.iter().zip(&after).filter(|(b, a)| b != a).count();
    assert_eq!(
        changed, 1,
        "only the accession item is added: {before:?} {after:?}"
    );
}

#[test]
fn join_refuses_another_orders_item_an_unaccepted_or_accessioned_item_and_another_matrix() {
    let f = fixture();
    let (order_id, accession) = accessioned(&f);
    let (_, other_items) = f.accepted_order(&[f.urine_test]);
    let requested = add_on(&f, order_id, f.urine_test, false);
    let oral_fluid = add_on(&f, order_id, f.oral_fluid_test, true);
    let (_, elsewhere) = accessioned(&f);
    let elsewhere_item = items(&f, elsewhere)[0];
    let before = items(&f, accession);

    let other_order = join(&f, accession, other_items[0]).unwrap_err();
    assert_eq!(other_order.code(), ErrorCode::Validation);
    assert_eq!(other_order.field_errors()[0].code, "other_order");
    assert!(matches!(
        join(&f, accession, requested),
        Err(SpecimenError::OrderItemNotReady)
    ));
    assert!(matches!(
        join(&f, accession, before[0]),
        Err(SpecimenError::ItemAccessioned)
    ));
    let other_matrix = join(&f, accession, oral_fluid).unwrap_err();
    assert_eq!(other_matrix.field_errors()[0].code, "matrix_mismatch");
    assert!(matches!(
        join(&f, elsewhere, oral_fluid),
        Err(SpecimenError::Invalid(_))
    ));
    assert!(matches!(
        join(&f, accession, elsewhere_item),
        Err(SpecimenError::Invalid(_))
    ));
    assert_eq!(items(&f, accession), before);
}
