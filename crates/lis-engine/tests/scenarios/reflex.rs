//! Reflex decisions from a reviewed laboratory result (REQ-ORD-003,
//! REQ-RPT-005): on the M2 journey order, which authorizes a reflex to the
//! confirmation test, a reviewer records whether the condition is met. Met
//! activates exactly one follow-on, not met creates nothing, a replay is
//! idempotent, and every unverified decision is refused. Expected outcomes
//! from REQ-ORD-003's acceptance and `docs/planning/m3-design-brief.md` §1
//! (RPT-005). Synthetic data only.

use lis_analytical::RevisionIndicator;
use lis_auth::RoleCode;
use lis_core::{CommandError, ErrorCode};
use lis_engine::{
    GetOrderInput, GetReportWorkspaceInput, ItemKind, ListReflexTriggersInput, OrderDetail,
    RecordReflexDecisionInput, RecordResultDispositionInput, ReflexDecision, ResultOutcome,
    ResultRevisionId, SequenceRole, SetLaboratoryProfile, SubmitTechnicalReviewInput,
    TriggerAssessmentView, TriggerSource,
};
use lis_reporting::ReportCompleteness;
use lis_results::ReviewDecision;

use crate::fixture::NEW_PASSWORD;
use crate::m2_lab::{Lab, Role};
use crate::m2_review::revision;
use crate::m2_steps::{JOURNEY, gate_codes, line};

/// The journey laboratory with sample 0 imported, QC-accepted and approved.
async fn reviewed_lab() -> Lab {
    let lab = Lab::start_as_with_profile(
        RevisionIndicator::None,
        lis_engine::InstallationKind::Laboratory,
        Some(laboratory_profile()),
    )
    .await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    lab.approve_all(0).await.unwrap();
    lab
}

/// A decision on sample 0's confirmation trigger, resting on `evidence`.
fn decision(
    lab: &Lab,
    evidence: ResultRevisionId,
    decision: ReflexDecision,
) -> RecordReflexDecisionInput {
    RecordReflexDecisionInput {
        parent_item_id: lab.samples[0].item_id,
        orderable_id: lab.orderables.confirmation.0,
        result_revision_id: evidence,
        decision,
        sequence_role: (decision == ReflexDecision::ConditionMet)
            .then_some(SequenceRole::Confirmation),
        reason: "Synthetic reflex decision".to_owned(),
    }
}

/// Records `input` as the reviewer, who holds `review_results` only.
async fn decide(
    lab: &Lab,
    input: RecordReflexDecisionInput,
) -> Result<TriggerAssessmentView, CommandError> {
    lab.sign_in(Role::Reviewer).await;
    lab.engine().record_reflex_decision(input).await
}

async fn order(lab: &Lab) -> OrderDetail {
    lab.sign_in(Role::Accessioner).await;
    lab.engine()
        .get_order(GetOrderInput {
            order_id: lab.samples[0].order_id,
        })
        .await
        .unwrap()
}

fn count(lab: &Lab, sql: &str) -> i64 {
    lab.t().conn().query_row(sql, [], |row| row.get(0)).unwrap()
}

const DECISION_EVENTS: &str = "SELECT count(*) FROM audit_events \
    WHERE action = 'result.record_reflex_decision' AND stream = 'clinical'";

#[tokio::test]
async fn condition_met_activates_one_follow_on_and_a_replay_changes_nothing() {
    let lab = reviewed_lab().await;
    let evidence = revision(&lab, 0, "SYN-MOR").await;
    lab.sign_in(Role::Reviewer).await;
    let triggers = lab
        .engine()
        .list_reflex_triggers(ListReflexTriggersInput {
            specimen_id: lab.samples[0].specimen_id,
        })
        .await
        .unwrap();
    assert_eq!(triggers.len(), 1);
    assert_eq!(
        (triggers[0].parent_item_id, triggers[0].orderable_id),
        (lab.samples[0].item_id, lab.orderables.confirmation.0)
    );
    assert!(triggers[0].evidence_revision_ids.contains(&evidence));
    assert_eq!(triggers[0].allowed_actions, ["record_reflex_decision"]);

    let met = decision(&lab, evidence, ReflexDecision::ConditionMet);
    let first = decide(&lab, met.clone()).await.unwrap();
    assert_eq!(first.outcome, "activated");
    assert_eq!(first.trigger_source, TriggerSource::LaboratoryResult);
    assert_eq!(first.trigger_ref, evidence.to_string());
    let replay = decide(&lab, met).await.unwrap();
    assert_eq!(replay, first);
    assert_eq!(count(&lab, DECISION_EVENTS), 1, "a replay records nothing");

    // REQ-ORD-003: the opposite decision on the same trigger is refused.
    let refused = decide(
        &lab,
        decision(&lab, evidence, ReflexDecision::ConditionNotMet),
    )
    .await;
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Conflict);

    let order = order(&lab).await;
    let reflexes: Vec<_> = order
        .items
        .iter()
        .filter(|item| item.kind == ItemKind::Reflex)
        .collect();
    assert_eq!(reflexes.len(), 1);
    assert_eq!(first.created_item_id, Some(reflexes[0].item_id));
    assert_eq!(reflexes[0].sequence_role, Some(SequenceRole::Confirmation));
    assert_eq!(order.trigger_assessments, [first]);
    lab.sign_in(Role::Reviewer).await;
    let pending = lab
        .engine()
        .list_reflex_triggers(ListReflexTriggersInput {
            specimen_id: lab.samples[0].specimen_id,
        })
        .await
        .unwrap();
    assert!(pending.is_empty(), "{pending:?}");
}

/// REQ-RPT-005: the journey report is partial while the trigger is pending
/// and can be final once the reviewer records that the condition is not met.
#[tokio::test]
async fn condition_not_met_creates_nothing_and_lets_the_report_be_final() {
    let lab = reviewed_lab().await;
    let releaser = add_releaser(&lab).await;
    assert_eq!(
        completeness(&lab, &releaser).await,
        Some(ReportCompleteness::Partial)
    );
    let items_before = order(&lab).await.items.len();
    let evidence = revision(&lab, 0, "SYN-MOR").await;
    let input = decision(&lab, evidence, ReflexDecision::ConditionNotMet);
    let recorded = decide(&lab, input.clone()).await.unwrap();
    assert_eq!(
        (recorded.outcome.as_str(), recorded.created_item_id),
        ("condition_not_met", None)
    );
    assert_eq!(decide(&lab, input).await.unwrap(), recorded);
    assert_eq!(count(&lab, DECISION_EVENTS), 1);

    let order = order(&lab).await;
    assert_eq!(order.items.len(), items_before, "no item, charge or result");
    assert_eq!(order.trigger_assessments, [recorded]);
    assert_eq!(
        count(&lab, "SELECT count(*) FROM res_results"),
        6,
        "no negative result is created"
    );
    assert_eq!(
        completeness(&lab, &releaser).await,
        Some(ReportCompleteness::Final)
    );
}

/// Creates the releaser and returns its authenticator secret.
async fn add_releaser(lab: &Lab) -> String {
    lab.sign_in(Role::Admin).await;
    lab.t().create_user("rel1", &[RoleCode::Releaser]).await;
    let secret = lab.t().first_sign_in("rel1").await.1;
    resync(lab).await;
    secret
}

// Establish performing-site provenance before the fixture creates a run.
fn laboratory_profile() -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version: None,
        name: "Synthetic Reflex Laboratory".to_owned(),
        address_lines: vec!["1 Synthetic Road".to_owned()],
        clia_id: "00D0000001".to_owned(),
        director: "Dr. Synthetic".to_owned(),
        phone: "555-0199".to_owned(),
        time_zone: "America/Denver".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: None,
    }
}

/// The completeness a candidate prepared now would have, as the releaser sees it.
async fn completeness(lab: &Lab, releaser: &str) -> Option<ReportCompleteness> {
    lab.t().sign_in_with("rel1", NEW_PASSWORD, releaser).await;
    let workspace = lab
        .engine()
        .get_report_workspace(GetReportWorkspaceInput {
            specimen_id: lab.samples[0].specimen_id,
        })
        .await
        .unwrap();
    resync(lab).await;
    assert!(
        workspace.preview.gates.is_empty(),
        "{:?}",
        workspace.preview.gates
    );
    workspace.preview.completeness
}

/// Makes the desk's cached role true again after another account signed in.
async fn resync(lab: &Lab) {
    lab.sign_in(Role::Analyst).await;
    lab.sign_in(Role::Quality).await;
}

#[tokio::test]
async fn unverified_decisions_are_refused_and_record_nothing() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let evidence = revision(&lab, 0, "SYN-MOR").await;
    let not_met = || decision(&lab, evidence, ReflexDecision::ConditionNotMet);

    // The person states a reason, and the sequence role only for "met".
    let mut blank = not_met();
    blank.reason = "  ".to_owned();
    assert_code(decide(&lab, blank).await, ErrorCode::Validation);
    let mut no_role = decision(&lab, evidence, ReflexDecision::ConditionMet);
    no_role.sequence_role = None;
    assert_code(decide(&lab, no_role).await, ErrorCode::Validation);

    // No review, then a rejected (stale) review.
    assert_gate(decide(&lab, not_met()).await, "review_missing");
    lab.approve_all(0).await.unwrap();
    reject(&lab, evidence).await;
    assert_gate(decide(&lab, not_met()).await, "review_rejected");
    lab.approve_all(0).await.unwrap();

    // A result of another order's specimen, and an orderable out of scope.
    lab.approve_all(1).await.unwrap();
    let unrelated = revision(&lab, 1, "SYN-MOR").await;
    let foreign = decision(&lab, unrelated, ReflexDecision::ConditionNotMet);
    assert_code(decide(&lab, foreign).await, ErrorCode::GateBlocked);
    let mut out_of_scope = decision(&lab, evidence, ReflexDecision::ConditionMet);
    out_of_scope.orderable_id = lab.orderables.panel.0;
    assert_code(decide(&lab, out_of_scope).await, ErrorCode::GateBlocked);

    // A superseded revision.
    supersede_morphine(&lab).await;
    assert_code(decide(&lab, not_met()).await, ErrorCode::Conflict);

    let order = order(&lab).await;
    assert!(order.trigger_assessments.is_empty());
    assert!(order.items.iter().all(|item| item.kind != ItemKind::Reflex));
    assert_eq!(count(&lab, DECISION_EVENTS), 0);
}

fn assert_code<T: std::fmt::Debug>(result: Result<T, CommandError>, code: ErrorCode) {
    assert_eq!(result.unwrap_err().code(), code);
}

fn assert_gate<T: std::fmt::Debug>(result: Result<T, CommandError>, code: &str) {
    let error = result.unwrap_err();
    assert_eq!(error.code(), ErrorCode::GateBlocked, "{error:?}");
    assert!(gate_codes(&error).iter().any(|c| c == code), "{error:?}");
}

async fn reject(lab: &Lab, evidence: ResultRevisionId) {
    lab.sign_in(Role::Reviewer).await;
    lab.engine()
        .submit_technical_review(SubmitTechnicalReviewInput {
            specimen_id: lab.samples[0].specimen_id,
            decision: ReviewDecision::Rejected,
            result_revision_ids: vec![evidence],
            reason: Some("Synthetic rejection".to_owned()),
        })
        .await
        .unwrap();
}

/// Records a disposition, so the morphine line gets a new current revision.
async fn supersede_morphine(lab: &Lab) {
    let results = lab.results(0).await;
    let morphine = line(&results, "SYN-MOR");
    lab.engine()
        .record_result_disposition(RecordResultDispositionInput {
            result_id: Some(morphine.result_id),
            expected_revision: morphine.revision,
            component_id: None,
            analyte_id: None,
            specimen_id: None,
            outcome: ResultOutcome::Invalid,
            reason: "Synthetic invalid result".to_owned(),
        })
        .await
        .unwrap();
}
