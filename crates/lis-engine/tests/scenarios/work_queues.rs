//! Operational work queues through the real engine (REQ-OPS-004,
//! REQ-RES-004/005). Synthetic identifiers and accession numbers only.

use lis_analytical::{MaterialKind, QcOutcome, RegisterLotInput, RevisionIndicator};
use lis_core::{Clock, CommandError, ErrorCode, UserId};
use lis_engine::{
    Engine, ListLotsInput, ListWorkQueueItemsInput, PlaceClarificationHoldInput, WorkQueueItem,
};

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{JOURNEY, UNIT_MISMATCH, message};

pub(crate) async fn work_queues_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    Some(match name {
        "list_work_queues" => engine.list_work_queues().await.map(drop),
        "list_work_queue_items" => engine
            .list_work_queue_items(ListWorkQueueItemsInput {
                after: None,
                queue: "identity_holds".to_owned(),
                limit: 10,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn input(queue: &str, limit: u32) -> ListWorkQueueItemsInput {
    ListWorkQueueItemsInput {
        after: None,
        queue: queue.to_owned(),
        limit,
    }
}

async fn items(lab: &Lab, queue: &str) -> Vec<WorkQueueItem> {
    lab.engine()
        .list_work_queue_items(input(queue, 500))
        .await
        .unwrap()
        .items
}

fn assert_oldest_first(items: &[WorkQueueItem]) {
    assert!(
        items
            .windows(2)
            .all(|pair| (pair[0].since, &pair[0].item_id) <= (pair[1].since, &pair[1].item_id)),
        "queue is not oldest first: {items:?}"
    );
}

fn assert_phi_free_titles(items: &[WorkQueueItem]) {
    assert!(
        items
            .iter()
            .all(|item| !item.title.contains("SYNTHETIC") && !item.title.contains("1980-01-01"))
    );
}

#[tokio::test]
async fn new_owner_queues_list_items_and_review_filters_before_the_limit() {
    let mut lab = Lab::start(RevisionIndicator::None).await;

    lab.sign_in(Role::Accessioner).await;
    let held = lab
        .engine()
        .register_order(lab.order_input(lab.orderables.panel.1, None))
        .await
        .unwrap();
    lab.engine()
        .place_clarification_hold(PlaceClarificationHoldInput {
            order_id: held.order_id,
            expected_revision: held.revision,
            deficiency: lis_orders::ClarificationDeficiency::OtherAuthority,
            note: Some("Synthetic clarification needed".to_owned()),
        })
        .await
        .unwrap();
    for queue in ["orders_needing_decision", "orders_on_hold"] {
        let rows = items(&lab, queue).await;
        assert!(!rows.is_empty(), "{queue}");
        assert_oldest_first(&rows);
        assert_phi_free_titles(&rows);
    }

    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.approve_all(0).await.unwrap();
    lab.approve_all(1).await.unwrap();

    // Regression: the matching specimen is newer than more than `limit`
    // current-result specimens. Fully current reviews are not "ready".
    lab.sign_in(Role::Reviewer).await;
    let ready = lab
        .engine()
        .list_work_queue_items(input("review_ready", 1))
        .await
        .unwrap()
        .items;
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].item_id, lab.samples[2].specimen_id.to_string());
    assert!(
        ready
            .iter()
            .all(|row| row.item_id != lab.samples[0].specimen_id.to_string())
    );
    for queue in ["review_stale", "review_blocked", "import_quarantine"] {
        let rows = items(&lab, queue).await;
        assert!(rows.is_empty(), "unexpected item in {queue}: {rows:?}");
    }

    lab.import(UNIT_MISMATCH).await;
    let confirmation = lab.add_sample(lab.orderables.confirmation.1).await;
    lab.place(&confirmation).await;
    let label = confirmation.label_code.clone();
    lab.samples.push(confirmation);
    lab.import(&message(&[
        "P|1",
        &format!("O|1|{label}|SYN-POS-WQ||R"),
        "R|1|^^^SYN-MOR|60.0|ng/mL||H||F||SYN-TECH-01||20261013135500|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|70.0|ng/mL||H||F||SYN-TECH-01||20261013135500|SYNTH-LCMS-01",
    ]))
    .await;

    lab.assess_qc(lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    lab.sign_in(Role::Reviewer).await;
    for queue in [
        "review_stale",
        "review_blocked",
        "import_quarantine",
        "performed_not_ordered",
    ] {
        let rows = items(&lab, queue).await;
        assert!(!rows.is_empty(), "{queue}");
        assert_oldest_first(&rows);
        assert_phi_free_titles(&rows);
    }
}

#[tokio::test]
async fn hidden_queue_is_forbidden_explicitly_and_unknown_queue_is_validation() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.sign_in(Role::Reviewer).await;
    let summaries = lab.engine().list_work_queues().await.unwrap();
    assert!(
        summaries
            .iter()
            .all(|summary| summary.queue != "orders_needing_decision")
    );

    let forbidden = lab
        .engine()
        .list_work_queue_items(input("orders_needing_decision", 10))
        .await
        .unwrap_err();
    assert_eq!(forbidden.code(), ErrorCode::Forbidden);

    let unknown = lab
        .engine()
        .list_work_queue_items(input("not_a_queue", 10))
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), ErrorCode::Validation);
    let json = serde_json::to_value(unknown).unwrap();
    assert_eq!(json["fieldErrors"][0]["field"], "queue");
}

#[tokio::test]
async fn summary_marks_a_count_that_reaches_the_bound() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.sign_in(Role::Accessioner).await;
    for _ in 0..500 {
        lab.engine()
            .register_order(lab.order_input(lab.orderables.panel.1, None))
            .await
            .unwrap();
    }
    let summaries = lab.engine().list_work_queues().await.unwrap();
    let orders = summaries
        .iter()
        .find(|summary| summary.queue == "orders_needing_decision")
        .unwrap();
    assert_eq!(orders.count, Some(500));
    assert!(orders.count_capped);
}

// REQ-OPS-013: a position awaiting results is named `<run>:<position>`, so the work-queue
// screen can open its run, as a QC item is named `<run>:<analyte>`.
#[tokio::test]
async fn positions_awaiting_results_are_named_by_their_run_and_position() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.sign_in(Role::Analyst).await;
    let rows = items(&lab, "results_expected").await;
    assert!(!rows.is_empty());
    let run = lab.run.to_string();
    for row in &rows {
        assert_eq!(row.item_kind, "run_position");
        let (named_run, position) = row.item_id.split_once(':').unwrap();
        assert_eq!((named_run, position.len()), (run.as_str(), run.len()));
    }
}

// #4557: an active lot inside the laboratory's warning period (14 days by default) is listed
// and marked in the lot list; one beyond it is not.
#[tokio::test]
async fn lots_expiring_soon_lists_active_lots_inside_the_warning_period() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.sign_in(Role::Quality).await;
    let today: jiff::civil::Date = lab.t().clock.now().to_string()[..10].parse().unwrap();
    let mut registered = Vec::new();
    for (number, days) in [("LOT-SOON-3", 3), ("LOT-LATE-15", 15)] {
        let expires = today.checked_add(jiff::Span::new().days(days)).unwrap();
        let lot = lab
            .engine()
            .register_lot(RegisterLotInput {
                material_kind: MaterialKind::Control,
                product_name: "Synthetic control".to_owned(),
                lot_number: number.to_owned(),
                level_text: None,
                sample_label: Some(format!("QC-{number}")),
                expires_on: expires.to_string(),
            })
            .await
            .unwrap();
        registered.push(lot.lot_id);
    }

    let rows = items(&lab, "lots_expiring_soon").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (
            rows[0].item_kind.as_str(),
            rows[0].item_id.clone(),
            rows[0].title.as_str()
        ),
        ("lot", registered[0].to_string(), "LOT-SOON-3")
    );
    assert_eq!(rows[0].state_label.as_deref(), Some("Expires in 3 days"));
    assert_eq!(rows[0].next_action, "Register a new lot");

    let lots = lab
        .engine()
        .list_lots(ListLotsInput { active_only: true })
        .await
        .unwrap();
    let marked = |id| {
        lots.iter()
            .find(|lot| lot.lot_id == id)
            .unwrap()
            .expiring_soon
    };
    assert!(marked(registered[0]));
    assert!(!marked(registered[1]));
}
