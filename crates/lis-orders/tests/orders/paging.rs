//! Regression of the real SQLite order directory across page limits and clock rollback.
use super::*;

#[test]
fn order_pages_reach_all_equal_timestamp_and_rolled_back_records() {
    let f = Fixture::open();
    let mut expected = Vec::new();
    for _ in 0..205 {
        let order = f.register(None);
        expected.push((order.received_at, order.order_id));
    }
    f.t.clock
        .set("2020-01-01T00:00:00.000000000Z".parse().unwrap());
    let old_clock = f.register(None);
    expected.push((old_clock.received_at, old_clock.order_id));
    expected.sort_by(|a, b| b.cmp(a));
    let mut before_order_id = None;
    let mut seen = Vec::new();
    loop {
        let input = ListOrdersInput {
            filter: OrderFilter::All,
            subject_id: Some(f.subject_id),
            limit: 200,
            before_received_at: None,
            before_order_id,
        };
        let page =
            f.t.db
                .read_blocking(move |conn| lis_orders::list_orders(conn, &input))
                .unwrap();
        seen.extend(page.iter().map(|r| r.order_id));
        if page.len() < 200 {
            break;
        }
        before_order_id = page.last().map(|r| r.order_id);
    }
    assert_eq!(seen, expected.iter().map(|(_, id)| *id).collect::<Vec<_>>());
    assert_eq!(seen.len(), 206);
    assert_eq!(seen.last(), Some(&old_clock.order_id));
}
