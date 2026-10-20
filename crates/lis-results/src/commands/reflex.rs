//! Reflex decisions from a verified laboratory result (REQ-ORD-003,
//! REQ-RPT-005).
//!
//! The person decides whether a reflex condition is met; nothing here infers
//! it from the cutoff classification. In the caller's transaction the result
//! revision must be current and carry a current approved technical review,
//! and its order must be the parent item's order. Orders then applies its own
//! scope, trigger-state and replay checks through its laboratory-result entry
//! points, so a replay returns the recorded decision and a conflicting one is
//! refused.

use lis_catalog::OrderableId;
use lis_orders::{OrderDetail, OrderItemId, TriggerAssessmentView};
use serde::Serialize;

use super::*;
use crate::queries::{get_specimen_results, review_status};
use crate::views::ReviewStatus;

lis_core::text_enum!(/// A person's decision on a laboratory-result reflex trigger.
    ReflexDecision { ConditionMet = "condition_met", ConditionNotMet = "condition_not_met", });

/// The only action a pending trigger offers.
const RECORD_REFLEX_DECISION: &str = "record_reflex_decision";

/// Records whether a reflex condition is met on a reviewed result revision.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordReflexDecisionInput {
    pub parent_item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub result_revision_id: ResultRevisionId,
    pub decision: ReflexDecision,
    /// Testing sequence role of the activated item, supplied by the person
    /// when the condition is met and never inferred; absent otherwise.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub sequence_role: Option<SequenceRole>,
    pub reason: String,
}

/// An unresolved reflex trigger of a specimen's order items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReflexTriggerView {
    pub parent_item_id: OrderItemId,
    pub orderable_id: OrderableId,
    pub orderable_code: String,
    pub orderable_name: String,
    /// Current revisions of this specimen with a current approved review;
    /// the person chooses the one the decision rests on.
    pub evidence_revision_ids: Vec<ResultRevisionId>,
    pub allowed_actions: Vec<String>,
}

/// Records a person's reflex decision through orders' verified entry points.
pub fn record_reflex_decision(
    ctx: &mut CommandContext<'_>,
    input: RecordReflexDecisionInput,
) -> Result<TriggerAssessmentView, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    require_user(ctx)?;
    require_reason("reason", &input.reason)?;
    let plan = plan(&input)?;
    let (order_id, _, order_revision) = reflex_parent(ctx.tx(), input.result_revision_id)?;
    match review_status(ctx.tx(), input.result_revision_id)? {
        ReviewStatus::Current { .. } => {}
        ReviewStatus::Stale { reasons } => return Err(ResultError::GateBlocked(reasons)),
        ReviewStatus::None => {
            return Err(ResultError::GateBlocked(vec![crate::error::gate(
                "review_missing",
                "The result needs an approved technical review first.",
            )]));
        }
    }
    let detail = apply_decision(ctx, &input, plan, order_id, order_revision)?;
    // Orders leaves the order revision unchanged on an idempotent replay.
    if detail.revision != order_revision {
        ctx.audit(
            clinical("result.record_reflex_decision")
                .record("result_revision", input.result_revision_id)
                .reason(input.reason.clone())
                .detail("decision", AuditDetail::code(input.decision.as_str()))
                .detail("order", AuditDetail::id(order_id)),
        )?;
    }
    detail
        .trigger_assessments
        .into_iter()
        .find(|a| a.parent_item_id == input.parent_item_id && a.orderable_id == input.orderable_id)
        .ok_or(ResultError::NotFound("trigger assessment"))
}

/// The decision with the sequence role it needs, validated before any lookup.
#[derive(Clone, Copy)]
enum Plan {
    Activate(SequenceRole),
    NotMet,
}

fn plan(input: &RecordReflexDecisionInput) -> Result<Plan, ResultError> {
    match (input.decision, input.sequence_role) {
        (ReflexDecision::ConditionMet, Some(role)) => Ok(Plan::Activate(role)),
        (ReflexDecision::ConditionNotMet, None) => Ok(Plan::NotMet),
        (ReflexDecision::ConditionMet, None) => Err(crate::error::invalid(
            "sequenceRole",
            "required",
            "Choose the testing sequence role of the reflex test.",
        )),
        (ReflexDecision::ConditionNotMet, Some(_)) => Err(crate::error::invalid(
            "sequenceRole",
            "not_allowed",
            "A sequence role applies only when the condition is met.",
        )),
    }
}

fn apply_decision(
    ctx: &mut CommandContext<'_>,
    input: &RecordReflexDecisionInput,
    plan: Plan,
    order_id: lis_orders::OrderId,
    order_revision: i64,
) -> Result<OrderDetail, ResultError> {
    let result_revision_id = input
        .result_revision_id
        .to_string()
        .parse()
        .map_err(|_| ResultError::WrongState("result revision"))?;
    Ok(match plan {
        Plan::Activate(sequence_role) => {
            let version = lis_catalog::list_active_orderables(ctx.tx())?
                .into_iter()
                .find(|version| version.orderable_id == input.orderable_id)
                .ok_or(ResultError::NotFound("active orderable"))?;
            lis_orders::request_follow_on_from_result(
                ctx,
                lis_orders::LaboratoryResultFollowOnInput {
                    order_id,
                    expected_revision: order_revision,
                    orderable_version_id: version.id,
                    sequence_role,
                    parent_item_id: input.parent_item_id,
                    result_revision_id,
                    reason: input.reason.clone(),
                },
            )?
        }
        Plan::NotMet => lis_orders::record_trigger_not_met_from_result(
            ctx,
            lis_orders::LaboratoryResultTriggerNotMetInput {
                order_id,
                parent_item_id: input.parent_item_id,
                orderable_id: input.orderable_id,
                result_revision_id,
                reason: input.reason.clone(),
            },
        )?,
    })
}

/// Reflex triggers of the specimen's order items with no recorded decision,
/// as the report manifest counts them `trigger_pending` (REQ-RPT-005).
pub fn pending_reflex_triggers(
    conn: &rusqlite::Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<ReflexTriggerView>, ResultError> {
    let accession =
        lis_specimens::get_accession(conn, lis_specimens::specimen_accession(conn, specimen_id)?)?;
    let order = lis_orders::get_order(conn, accession.order_id)?;
    let evidence: Vec<ResultRevisionId> = get_specimen_results(conn, specimen_id)?
        .lines
        .into_iter()
        .filter(|line| matches!(line.review_status, ReviewStatus::Current { .. }))
        .filter_map(|line| line.current.map(|current| current.revision_id))
        .collect();
    let orderables = lis_catalog::list_orderables(conn)?;
    let mut triggers = Vec::new();
    for item in order
        .items
        .iter()
        .filter(|item| accession.order_item_ids.contains(&item.item_id))
    {
        for orderable_id in &item.reflex_scope_orderable_ids {
            if order
                .trigger_assessments
                .iter()
                .any(|a| a.parent_item_id == item.item_id && a.orderable_id == *orderable_id)
            {
                continue;
            }
            let orderable = orderables
                .iter()
                .find(|o| o.id == *orderable_id)
                .ok_or(ResultError::NotFound("orderable"))?;
            triggers.push(ReflexTriggerView {
                parent_item_id: item.item_id,
                orderable_id: *orderable_id,
                orderable_code: orderable.code.clone(),
                orderable_name: orderable.name.clone(),
                evidence_revision_ids: evidence.clone(),
                allowed_actions: if evidence.is_empty() {
                    Vec::new()
                } else {
                    vec![RECORD_REFLEX_DECISION.to_owned()]
                },
            });
        }
    }
    Ok(triggers)
}
