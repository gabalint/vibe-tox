//! REQ-ORD-002: ordered labels remain available after catalog retirement.

use super::*;

#[test]
fn an_order_keeps_its_catalog_name_and_code_after_the_version_is_retired() {
    let fixture = Fixture::open();
    let order = prepare_accepted_order(&fixture);
    assert_eq!(order.items[0].orderable_code, "SYN-O");
    assert_eq!(order.items[0].orderable_name, "Synthetic orderable");
    let version_id = fixture.orderable_version_id;
    fixture
        .t
        .db
        .write_blocking(
            user(UserId::generate(), &[Capability::ApproveConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                lis_catalog::retire_orderable_version(
                    ctx,
                    lis_catalog::RetireOrderableVersion {
                        orderable_version_id: version_id,
                        expected_revision: 2,
                        reason: "Synthetic catalog replacement".to_owned(),
                    },
                )
            },
        )
        .unwrap();
    let order_id = order.order_id;
    let later = fixture
        .t
        .db
        .read_blocking(move |conn| {
            assert!(lis_catalog::list_active_orderables(conn)?.is_empty());
            lis_orders::get_order(conn, order_id)
        })
        .unwrap();
    assert_eq!(later.items[0].orderable_version_id, version_id);
    assert_eq!(later.items[0].orderable_code, "SYN-O");
    assert_eq!(later.items[0].orderable_name, "Synthetic orderable");
}
