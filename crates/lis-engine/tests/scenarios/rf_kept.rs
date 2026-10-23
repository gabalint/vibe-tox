//! Analytes outside the ordered scope through the engine (result files
//! brief F17, Q-41): a file row for an analyte the order does not include is
//! kept as a measurement, with no result line and no Not ordered queue
//! item; an accepted add-on pulls it in with `use_measured_result`, and the
//! result then goes through technical review and release. ASTM rows outside
//! the ordered scope still go to the Not ordered queue. Synthetic data only.

use lis_analytical::AttemptKind;
use lis_catalog::OrderableVersionId;
use lis_core::{CommandError, ErrorCode};
use lis_engine::{
    AddPatientPositionInput, CreateAccessionInput, DecideFollowOnInput, Decision, GetOrderInput,
    ItemKind, ListWorkQueueItemsInput, RequestFollowOnInput, SequenceRole, SpecimenResults,
    UseMeasuredResultInput, WorkQueueItem,
};
use lis_orders::{ItemState, OrderComponentId, OrderItemId};
use lis_results::{MeasurementForm, ResultCause};
use lis_specimens::{CollectionInput, CollectionTimeInput, SpecimenInput, VolumeKind};

use crate::m2_config::URINE;
use crate::m2_lab::{Role, Sample};
use crate::m2_steps::{line, message};
use crate::rf_lab::{FileLab, control_rows, row};

/// The accessioner requests `test` as an add-on of `sample`'s order and
/// accepts it; returns the add-on item and its component.
async fn add_on(
    f: &FileLab,
    sample: &Sample,
    test: OrderableVersionId,
) -> (OrderItemId, OrderComponentId) {
    f.lab.sign_in(Role::Accessioner).await;
    let e = f.engine();
    let order_id = sample.order_id;
    let order = e.get_order(GetOrderInput { order_id }).await.unwrap();
    let requested = e
        .request_follow_on(RequestFollowOnInput {
            order_id,
            expected_revision: order.revision,
            kind: ItemKind::AddOn,
            orderable_version_id: test,
            sequence_role: SequenceRole::FirstLine,
            parent_item_id: None,
            authority_reference: Some("SYN-ADDON-CALL".to_owned()),
            poc_result_id: None,
            trigger_source: None,
            trigger_ref: None,
            reason: "Synthetic clinician call".to_owned(),
        })
        .await
        .unwrap();
    let item = requested
        .items
        .iter()
        .find(|item| item.kind == ItemKind::AddOn && item.state == ItemState::Requested)
        .unwrap();
    let accepted = e
        .decide_follow_on(DecideFollowOnInput {
            item_id: item.item_id,
            expected_revision: item.revision,
            decision: Decision::Accept,
            reason: "Synthetic acceptance".to_owned(),
        })
        .await
        .unwrap();
    let component = accepted
        .items
        .iter()
        .find(|accepted| accepted.item_id == item.item_id)
        .unwrap()
        .components[0]
        .component_id;
    (item.item_id, component)
}

/// The reviewer's `use_measured_result`.
async fn use_measured(
    f: &FileLab,
    component: OrderComponentId,
    execution_id: lis_analytical::ExecutionId,
    reason: &str,
) -> Result<SpecimenResults, CommandError> {
    f.lab.sign_in(Role::Reviewer).await;
    f.engine()
        .use_measured_result(UseMeasuredResultInput {
            order_component_id: component,
            execution_id,
            reason: reason.to_owned(),
        })
        .await
}

/// The reviewer's Not ordered work queue (performed, not ordered).
async fn not_ordered_queue(f: &FileLab) -> Vec<WorkQueueItem> {
    f.lab.sign_in(Role::Reviewer).await;
    f.engine()
        .list_work_queue_items(ListWorkQueueItemsInput {
            after: None,
            queue: "performed_not_ordered".to_owned(),
            limit: 50,
        })
        .await
        .unwrap()
        .items
}

/// The first field error's code.
fn field_code(error: &CommandError) -> String {
    serde_json::to_value(error).unwrap()["fieldErrors"][0]["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn a_kept_analyte_is_used_for_an_accepted_add_on_then_reviewed_and_released() {
    let mut f = FileLab::start().await;
    let s = f.add_sample(f.tests.morphine).await;
    let label = s.label_code.as_str();
    let mut rows = control_rows();
    rows.push(row(label, "Unknown", "Morphine", "60.0"));
    rows.push(row(label, "Unknown", "Oxycodone", "7.5"));
    let attempt = f.import("batch.csv", &rows).await;
    assert_eq!(
        (
            attempt.rows_new,
            attempt.rows_retained,
            attempt.rows_quarantined
        ),
        (1, 1, 0)
    );
    assert!(not_ordered_queue(&f).await.is_empty());
    let results = f.lab.results(0).await;
    let kept = results.measured_not_ordered[0].clone();
    assert_eq!(
        (
            kept.analyte_code.as_str(),
            kept.value_text.as_deref(),
            kept.form
        ),
        ("SYN-OXY", Some("7.5"), MeasurementForm::Quantity)
    );
    assert!(kept.add_on_components.is_empty());

    let (_, component) = add_on(&f, &s, f.tests.oxycodone).await;
    let offered = f.lab.results(0).await.measured_not_ordered[0].clone();
    assert_eq!(offered.add_on_components[0].component_id, component);
    let unexplained = use_measured(&f, component, kept.execution_id, " ").await;
    assert_eq!(unexplained.unwrap_err().code(), ErrorCode::Validation);
    let used = use_measured(&f, component, kept.execution_id, "Synthetic add-on").await;
    let used = used.unwrap();
    assert!(used.measured_not_ordered.is_empty());
    let oxy = line(&used, "SYN-OXY").current.clone().unwrap();
    assert_eq!(
        (oxy.cause, oxy.magnitude.as_deref(), oxy.reason.as_deref()),
        (ResultCause::Selected, Some("7.5"), Some("Synthetic add-on"))
    );
    // The response names the reviewer who kept the result.
    let reviewer: String = f
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT display_name FROM auth_users WHERE id = ?1",
            [&oxy.created_by],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(oxy.created_by_display.as_deref(), Some(reviewer.as_str()));
    assert!(f.lab.t().events("clinical").contains(&(
        "result.use_measured_result".to_owned(),
        "succeeded".to_owned()
    )));

    f.lab.accept_qc().await;
    f.lab.approve_all(0).await.unwrap();
    let released = f.release(&s).await;
    assert_eq!(released.revision_no, 1);
    let lines: i64 = f
        .lab
        .t()
        .conn()
        .query_row("SELECT count(*) FROM rpt_v_released_lines", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(lines, 2, "morphine and the pulled-in oxycodone");

    astm_rows_outside_the_scope_still_go_to_the_not_ordered_queue(&f).await;
}

/// F17 leaves ASTM unchanged: an out-of-scope row is quarantined for the
/// Not ordered queue.
async fn astm_rows_outside_the_scope_still_go_to_the_not_ordered_queue(f: &FileLab) {
    let confirmation = f.lab.add_sample(f.lab.orderables.confirmation.1).await;
    f.lab.sign_in(Role::Analyst).await;
    f.engine()
        .add_patient_position(AddPatientPositionInput {
            run_id: f.astm.1,
            position_label: "ASTM-1".to_owned(),
            container_label: confirmation.label_code.clone(),
            attempt_kind: AttemptKind::Initial,
            related_execution_id: None,
            dilution_factor: None,
            prep_batch_id: None,
            reason: None,
        })
        .await
        .unwrap();
    let delivered = f
        .lab
        .deliver(&message(&[
            "P|1",
            &format!("O|1|{}|SYN-POS-F17||R", confirmation.label_code),
            "R|1|^^^SYN-MOR|60.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
            "R|2|^^^SYN-OXY|70.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
        ]))
        .await;
    let astm = f.attempt(delivered.receipt_id, 1).await;
    assert_eq!((astm.rows_quarantined, astm.rows_retained), (1, 0));
    assert_eq!(not_ordered_queue(f).await.len(), 1);
}

// F17 and §13: the owners refuse what the add-on cannot take, and a refused
// command writes nothing.
#[tokio::test]
async fn use_measured_result_refuses_items_it_cannot_fill() {
    let mut f = FileLab::start().await;
    let s = f.add_sample(f.tests.morphine).await;
    let t = f.add_sample(f.tests.morphine).await;
    f.import(
        "batch.csv",
        &[
            row(&s.label_code, "Unknown", "Morphine", "60.0"),
            row(&s.label_code, "Unknown", "Oxycodone", "7.5"),
            row(&t.label_code, "Unknown", "Morphine", "61.0"),
        ],
    )
    .await;
    let execution = f.lab.results(0).await.measured_not_ordered[0].execution_id;
    let (_, other_order) = add_on(&f, &t, f.tests.oxycodone).await;
    let (elsewhere_item, elsewhere) = add_on(&f, &s, f.tests.oxycodone).await;
    accession_elsewhere(&f, &s, elsewhere_item).await;
    let (_, not_served) = add_on(&f, &s, f.lab.orderables.panel.1).await;
    let before = f.lab.t().fingerprint();

    let refused = use_measured(&f, other_order, execution, "Synthetic").await;
    let refused = refused.unwrap_err();
    assert_eq!(
        (refused.code(), field_code(&refused).as_str()),
        (ErrorCode::Validation, "other_order")
    );
    let refused = use_measured(&f, elsewhere, execution, "Synthetic").await;
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Conflict);
    let refused = use_measured(&f, not_served, execution, "Synthetic").await;
    let refused = refused.unwrap_err();
    assert_eq!(
        (refused.code(), field_code(&refused).as_str()),
        (ErrorCode::Validation, "not_served")
    );
    let after = f.lab.t().fingerprint();
    let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
    assert!(
        added
            .iter()
            .all(|row| !row.starts_with("res_") && !row.starts_with("spc_")),
        "{added:?}"
    );
    let results = f.lab.results(0).await;
    assert_eq!(results.lines.len(), 1);
    assert_eq!(results.measured_not_ordered.len(), 1);
}

/// The accessioner accessions the add-on `item` of `sample`'s order with new material.
async fn accession_elsewhere(f: &FileLab, sample: &Sample, item: OrderItemId) {
    f.lab.sign_in(Role::Accessioner).await;
    f.engine()
        .create_accession(CreateAccessionInput {
            order_id: sample.order_id,
            order_item_ids: vec![item],
            collection: CollectionInput {
                time: CollectionTimeInput::Unknown,
                collector: None,
                observations: Vec::new(),
            },
            specimens: vec![SpecimenInput {
                specimen_type_code: URINE.to_owned(),
                container_count: 1,
                volume: None,
                volume_unit: None,
                volume_kind: VolumeKind::Unknown,
            }],
        })
        .await
        .unwrap();
}
