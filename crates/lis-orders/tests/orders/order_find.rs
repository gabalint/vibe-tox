//! The order worklist's find text: order number prefix, resolved matches and validation.
use super::*;

fn find(
    fixture: &Fixture,
    text: &str,
    matches: lis_orders::OrderMatches,
) -> Result<Vec<lis_orders::OrderId>, lis_orders::OrderError> {
    let input = ListOrdersInput {
        filter: OrderFilter::All,
        subject_id: None,
        limit: 50,
        before_received_at: None,
        before_order_id: None,
        search: Some(lis_core::Phi::new(text.to_owned())),
    };
    fixture.t.db.read_blocking(move |conn| {
        let orders = lis_orders::list_orders(conn, &input, &matches)?;
        Ok(orders.into_iter().map(|order| order.order_id).collect())
    })
}

#[test]
fn order_list_finds_by_order_number_start_or_resolved_matches() {
    let fixture = Fixture::open();
    let plain = fixture.register(None);
    let mut input = fixture.registration_input(None, ReflexAuthorization::Authorized);
    input.placer_order_id = Some("PO-SYN-0042".to_owned());
    input.placer_authority = Some("SYN-CLINIC".to_owned());
    let numbered = fixture
        .write(move |ctx| register_order(ctx, input))
        .unwrap();
    let none = lis_orders::OrderMatches::default;

    assert_eq!(
        find(&fixture, " po-syn-00", none()).unwrap(),
        [numbered.order_id]
    );
    assert!(find(&fixture, "SYN-0042", none()).unwrap().is_empty());
    assert!(find(&fixture, "PO-SYN-%", none()).unwrap().is_empty());
    let by_order = lis_orders::OrderMatches {
        order_ids: vec![plain.order_id],
        ..none()
    };
    assert_eq!(find(&fixture, "A26", by_order).unwrap(), [plain.order_id]);
    let by_subject = lis_orders::OrderMatches {
        subject_ids: vec![fixture.subject_id],
        ..none()
    };
    assert_eq!(
        find(&fixture, "Synthetic", by_subject).unwrap(),
        [numbered.order_id, plain.order_id]
    );
    let by_client = lis_orders::OrderMatches {
        client_ids: vec![fixture.client_id],
        ..none()
    };
    assert_eq!(find(&fixture, "Synth", by_client).unwrap().len(), 2);
    // Blank find text lists every order.
    assert_eq!(find(&fixture, "  ", none()).unwrap().len(), 2);
}

#[test]
fn order_list_refuses_find_text_over_200_characters() {
    let fixture = Fixture::open();
    let long = "A".repeat(201);
    let error = find(&fixture, &long, lis_orders::OrderMatches::default()).unwrap_err();
    assert_eq!(error.field_errors()[0].field, "search");
}
