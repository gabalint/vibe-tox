//! BILL-G03: attributable assignment/resolution/reopening, without clinical holds.

use lis_core::{Capability, CorrelationId, ErrorCode, Phi};
use lis_orders::{
    BillingReviewElement, BillingReviewStatus, CreateBillingReviewItemInput,
    ListBillingReviewItemsInput, UpdateBillingReviewItemInput, billing_review_items,
    create_billing_review_item, list_billing_review_items, order_billing_view,
    update_billing_review_item,
};
use lis_store::testing::principal;

use crate::Fixture;

fn opening(order_id: lis_orders::OrderId) -> CreateBillingReviewItemInput {
    CreateBillingReviewItemInput {
        order_id,
        element: BillingReviewElement::Diagnosis,
        note: Some("Synthetic: await source-supplied evidence".into()),
    }
}

fn resolve(item_id: lis_orders::BillingReviewItemId) -> UpdateBillingReviewItemInput {
    UpdateBillingReviewItemInput {
        item_id,
        expected_version: 1,
        status: BillingReviewStatus::Resolved,
        owner: Some("Synthetic billing team".into()),
        source_ref: Some(Phi::new("retained-source-record".into())),
        reason: Phi::new("Source supplied and retained".into()),
    }
}

#[test]
fn billing_staff_resolve_and_reopen_items_preserving_attribution_and_order_freshness() {
    let fixture = Fixture::open();
    let order = fixture.register(None);
    let id = order.order_id;
    let opened = fixture
        .t
        .db
        .write_blocking(
            principal(&[Capability::Billing]),
            CorrelationId::generate(),
            move |ctx| create_billing_review_item(ctx, opening(id)),
        )
        .unwrap();
    let item_id = opened.billing_review_items[0].item_id;
    let closed = fixture
        .write(move |ctx| update_billing_review_item(ctx, resolve(item_id)))
        .unwrap();
    assert_eq!(closed.status, BillingReviewStatus::Resolved);
    assert_eq!(closed.version, 2);
    assert!(closed.updated_by.is_some());
    let mut reopen = resolve(item_id);
    reopen.expected_version = 2;
    reopen.status = BillingReviewStatus::Open;
    reopen.reason = Phi::new("Supplied evidence needs clarification".into());
    let reopened = fixture
        .write(move |ctx| update_billing_review_item(ctx, reopen))
        .unwrap();
    assert_eq!(reopened.version, 3);
    assert_eq!(reopened.opened_at, closed.opened_at);
    let view = fixture
        .t
        .db
        .read_blocking(move |conn| order_billing_view(conn, &principal(&[Capability::Billing]), id))
        .unwrap();
    assert!(
        view.readiness_issues
            .contains(&"billing_review_open".into())
    );
    assert_eq!(view.order_revision, order.revision + 3);
    let reasons: Vec<String> = fixture
        .t
        .db
        .read_blocking(|conn| {
            let mut statement = conn.prepare(
                "SELECT reason FROM audit_events WHERE action='order.update_billing_review' ORDER BY seq",
            )?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<_>>()
                .map_err(lis_orders::OrderError::from)
        })
        .unwrap();
    assert_eq!(
        reasons,
        [
            "Source supplied and retained",
            "Supplied evidence needs clarification"
        ]
    );
    for sql in [
        "UPDATE ord_billing_review_actions SET status='open'",
        "DELETE FROM ord_billing_review_actions",
    ] {
        assert!(
            fixture
                .write(move |ctx| Ok(ctx.tx().execute(sql, [])?))
                .is_err()
        );
    }
}

#[test]
fn review_resolution_requires_owner_evidence_authority_and_current_version() {
    let fixture = Fixture::open();
    let order = fixture.register(None);
    let opened = fixture
        .write(move |ctx| create_billing_review_item(ctx, opening(order.order_id)))
        .unwrap();
    let item_id = opened.billing_review_items[0].item_id;
    for missing_owner in [true, false] {
        let mut incomplete = resolve(item_id);
        if missing_owner {
            incomplete.owner = None;
        } else {
            incomplete.source_ref = None;
        }
        assert_eq!(
            fixture
                .write(move |ctx| update_billing_review_item(ctx, incomplete))
                .unwrap_err()
                .code(),
            ErrorCode::Validation
        );
    }
    let denied = fixture
        .t
        .db
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| update_billing_review_item(ctx, resolve(item_id)),
        )
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
    assert!(matches!(
        denied,
        lis_orders::OrderError::Store(lis_store::StoreError::Denied(_))
    ));
    fixture
        .write(move |ctx| update_billing_review_item(ctx, resolve(item_id)))
        .unwrap();
    assert_eq!(
        fixture
            .write(move |ctx| update_billing_review_item(ctx, resolve(item_id)))
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
}

#[test]
fn financial_queue_is_authorized_bounded_and_pages_without_repeating_items() {
    let fixture = Fixture::open();
    let order = fixture.register(None);
    let id = order.order_id;
    for _ in 0..3 {
        fixture
            .write(move |ctx| create_billing_review_item(ctx, opening(id)))
            .unwrap();
    }
    let first = fixture
        .t
        .db
        .read_blocking(|conn| {
            list_billing_review_items(
                conn,
                &principal(&[Capability::Billing]),
                ListBillingReviewItemsInput {
                    status: Some(BillingReviewStatus::Open),
                    limit: 2,
                    after_item_id: None,
                },
            )
        })
        .unwrap();
    assert_eq!(first.len(), 2);
    let cursor = first[1].id;
    let second = fixture
        .t
        .db
        .read_blocking(move |conn| {
            list_billing_review_items(
                conn,
                &principal(&[Capability::Billing]),
                ListBillingReviewItemsInput {
                    status: Some(BillingReviewStatus::Open),
                    limit: 2,
                    after_item_id: Some(cursor),
                },
            )
        })
        .unwrap();
    assert_eq!(second.len(), 1);
    assert!(!first.iter().any(|item| item.id == second[0].id));
    let item_id = second[0].id;
    fixture
        .write(move |ctx| update_billing_review_item(ctx, resolve(item_id)))
        .unwrap();
    let effective = fixture
        .t
        .db
        .read_blocking(move |conn| billing_review_items(conn, id))
        .unwrap();
    assert_eq!(
        effective
            .iter()
            .filter(|item| item.status == BillingReviewStatus::Resolved)
            .count(),
        1
    );
}

#[test]
fn financial_queue_refuses_unauthorized_unbounded_and_missing_cursors() {
    let fixture = Fixture::open();
    let denied = fixture
        .t
        .db
        .read_blocking(|conn| {
            list_billing_review_items(
                conn,
                &principal(&[Capability::ViewAudit]),
                ListBillingReviewItemsInput {
                    status: None,
                    limit: 10,
                    after_item_id: None,
                },
            )
        })
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
    let oversized = fixture
        .t
        .db
        .read_blocking(|conn| {
            list_billing_review_items(
                conn,
                &principal(&[Capability::Billing]),
                ListBillingReviewItemsInput {
                    status: None,
                    limit: 101,
                    after_item_id: None,
                },
            )
        })
        .unwrap_err();
    assert_eq!(oversized.code(), ErrorCode::Validation);
    let absent = fixture
        .t
        .db
        .read_blocking(|conn| {
            list_billing_review_items(
                conn,
                &principal(&[Capability::Billing]),
                ListBillingReviewItemsInput {
                    status: None,
                    limit: 10,
                    after_item_id: Some(lis_orders::BillingReviewItemId::generate()),
                },
            )
        })
        .unwrap_err();
    assert_eq!(absent.code(), ErrorCode::NotFound);
}
