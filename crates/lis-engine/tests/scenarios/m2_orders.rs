//! M2 order decisions against analysis and review (REQ-ORD-002,
//! REQ-ORD-003, REQ-ORD-006, REQ-RES-005, NFR-01, NFR-02): cancellation
//! racing import and review, and standing-order acceptance. Reflex decisions
//! from a laboratory result are in `reflex.rs`.
//! Expected outcomes from `docs/planning/m2-design-brief.md` §1, §2.1, §3.2
//! and §3.6. Synthetic data only.

use lis_analytical::RevisionIndicator;
use lis_core::ErrorCode;
use lis_engine::{
    AcceptOrderInput, DecideCancellationInput, Decision, GetOrderInput, OrderStatus,
    RegisterStandingOrderInput, RequestCancellationInput,
};
use lis_orders::ClarificationDeficiency;

use crate::m2_lab::{Lab, Role};
use crate::m2_review::assert_refused;
use crate::m2_steps::{JOURNEY, current_revisions, line};

impl Lab {
    /// Cancels the item of sample `index`, as requested by the client and
    /// accepted by the accessioner.
    async fn cancel(&self, index: usize) {
        self.sign_in(Role::Accessioner).await;
        let e = self.engine();
        let requested = e
            .request_cancellation(RequestCancellationInput {
                item_id: self.samples[index].item_id,
                requested_by: "Synthetic Clinic front desk".to_owned(),
                reason: "Synthetic duplicate requisition".to_owned(),
            })
            .await
            .unwrap();
        let request = requested.cancellations.last().unwrap();
        e.decide_cancellation(DecideCancellationInput {
            request_id: request.request_id,
            expected_revision: request.revision,
            decision: Decision::Accept,
            reason: "Synthetic confirmed with the client".to_owned(),
        })
        .await
        .unwrap();
    }
}

/// Brief §1 (ORD-002) and §3.2: evidence that arrives after a cancellation
/// is kept but cannot pass review; a cancellation after review is reported
/// by the review gates that M3 release rechecks (§3.6).
#[tokio::test]
async fn a_cancelled_component_keeps_its_evidence_but_cannot_pass_review() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.cancel(0).await;
    let attempt = lab.import(JOURNEY).await;
    assert_eq!((attempt.rows_new, attempt.rows_quarantined), (6, 0));
    lab.accept_qc().await;
    let cancelled = lab.results(0).await;
    assert_eq!(cancelled.lines.len(), 2);
    for code in ["SYN-MOR", "SYN-OXY"] {
        let gates = &line(&cancelled, code).gates;
        assert!(
            gates.iter().any(|g| g.code == "component_cancelled"),
            "{gates:?}"
        );
    }
    assert_refused(lab.approve_all(0).await, "component_cancelled");

    lab.approve_all(1).await.unwrap();
    lab.cancel(1).await;
    let reviewed = lab.results(1).await;
    for code in ["SYN-MOR", "SYN-OXY"] {
        let gates = &line(&reviewed, code).gates;
        assert!(
            gates.iter().any(|g| g.code == "component_cancelled"),
            "{gates:?}"
        );
    }
    assert_refused(
        lab.approve(1, current_revisions(&reviewed)).await,
        "component_cancelled",
    );
}

/// Accepted authority remains patient/practitioner/scope-specific. An out-of-scope
/// request commits a hold; the intake clock does not determine collection validity.
#[tokio::test]
async fn an_out_of_scope_standing_order_leaves_a_retained_hold() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let order = standing_order(
        &lab,
        vec![lab.orderables.confirmation.0],
        "2026-10-01",
        "2026-12-31",
    )
    .await;
    let e = lab.engine();
    let held = e
        .accept_order(AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: order.revision,
        })
        .await
        .unwrap();
    assert_eq!(held.status, OrderStatus::Received);
    assert_eq!(held.holds.len(), 1);
    assert_eq!(
        held.holds[0].deficiency,
        ClarificationDeficiency::StandingOrderOutOfScope
    );
    assert_eq!(held.holds[0].origin, "standing_order_check");
    assert_eq!(held.holds[0].state, "open");
    let read = e
        .get_order(GetOrderInput {
            order_id: order.order_id,
        })
        .await
        .unwrap();
    assert_eq!(read.holds, held.holds);
    let again = e
        .accept_order(AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: read.revision,
        })
        .await;
    assert_eq!(again.unwrap_err().code(), ErrorCode::GateBlocked);
    let holds = lab
        .t()
        .events("clinical")
        .into_iter()
        .filter(|(action, _)| action == "order.hold")
        .count();
    assert_eq!(holds, 1);
}

#[tokio::test]
async fn delayed_intake_reserves_standing_authority_without_inventing_collection() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let order = standing_order(
        &lab,
        vec![lab.orderables.panel.0],
        "2026-01-01",
        "2026-06-30",
    )
    .await;
    let accepted = lab
        .engine()
        .accept_order(AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: order.revision,
        })
        .await
        .unwrap();
    assert_eq!(accepted.status, OrderStatus::Accepted);
    assert!(accepted.holds.is_empty());
    let standing = lab
        .engine()
        .list_standing_orders(lis_engine::ListStandingOrdersInput {
            subject_id: lab.parties.subject,
        })
        .await
        .unwrap();
    assert_eq!(standing.len(), 1);
    assert_eq!(
        Some(standing[0].standing_order_id),
        accepted.standing_order_id
    );
    assert_eq!(standing[0].accepted_uses, 1);
    // Acceptance reserves the use. Actual testing/release must still check the
    // original collection evidence; the specimen standing-authority tests exercise
    // unknown, valid earlier and corrected outside-period evidence.
    let accessions: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM spc_accessions WHERE order_id=?1",
            [order.order_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(accessions, 0);
}

async fn standing_order(
    lab: &Lab,
    scope: Vec<lis_catalog::OrderableId>,
    from: &str,
    until: &str,
) -> lis_engine::OrderDetail {
    lab.sign_in(Role::Accessioner).await;
    let e = lab.engine();
    let standing = e
        .register_standing_order(RegisterStandingOrderInput {
            subject_id: lab.parties.subject,
            practitioner_id: lab.parties.practitioner,
            scope_orderable_ids: scope,
            valid_from: from.to_owned(),
            valid_until: until.to_owned(),
            visit_limit: None,
            document_ref: "SYN-STANDING-M2".to_owned(),
        })
        .await
        .unwrap();
    e.register_order(lab.order_input(lab.orderables.panel.1, Some(standing.standing_order_id)))
        .await
        .unwrap()
}
