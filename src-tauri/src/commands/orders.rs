//! Thin order-entry IPC adapters (REQ-ORD-001–003, 005–007).

use lis_core::CommandError;
use lis_engine::{
    AcceptOrderInput, AmendOrderEvidenceInput, CreateBillingReviewItemInput,
    DecideCancellationInput, DecideFollowOnInput, Engine, GetOrderInput, ListOrdersInput,
    ListStandingOrdersInput, OrderDetail, OrderSummary, PlaceClarificationHoldInput,
    RecordTriggerNotMetInput, RegisterOrderInput, RegisterStandingOrderInput, RejectOrderInput,
    RequestCancellationInput, RequestFollowOnInput, ResolveClarificationHoldInput,
    StandingOrderView, TriggerSource,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PublicTriggerSource {
    ClientPointOfCare,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RequestFollowOnIpcInput {
    order_id: lis_engine::OrderId,
    expected_revision: i64,
    kind: lis_engine::ItemKind,
    orderable_version_id: lis_core::Id<lis_engine::OrderableVersion>,
    sequence_role: lis_engine::SequenceRole,
    parent_item_id: Option<lis_engine::OrderItemId>,
    authority_reference: Option<String>,
    poc_result_id: Option<lis_engine::OrderEvidenceId>,
    trigger_source: Option<PublicTriggerSource>,
    trigger_ref: Option<String>,
    reason: String,
}

impl From<RequestFollowOnIpcInput> for RequestFollowOnInput {
    fn from(input: RequestFollowOnIpcInput) -> Self {
        Self {
            order_id: input.order_id,
            expected_revision: input.expected_revision,
            kind: input.kind,
            orderable_version_id: input.orderable_version_id,
            sequence_role: input.sequence_role,
            parent_item_id: input.parent_item_id,
            authority_reference: input.authority_reference,
            poc_result_id: input.poc_result_id,
            trigger_source: input
                .trigger_source
                .map(|PublicTriggerSource::ClientPointOfCare| TriggerSource::ClientPointOfCare),
            trigger_ref: input.trigger_ref,
            reason: input.reason,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecordTriggerNotMetIpcInput {
    parent_item_id: lis_engine::OrderItemId,
    orderable_id: lis_engine::OrderableId,
    trigger_source: PublicTriggerSource,
    trigger_ref: String,
    reason: String,
}

impl From<RecordTriggerNotMetIpcInput> for RecordTriggerNotMetInput {
    fn from(input: RecordTriggerNotMetIpcInput) -> Self {
        Self {
            parent_item_id: input.parent_item_id,
            orderable_id: input.orderable_id,
            trigger_source: match input.trigger_source {
                PublicTriggerSource::ClientPointOfCare => TriggerSource::ClientPointOfCare,
            },
            trigger_ref: input.trigger_ref,
            reason: input.reason,
        }
    }
}

#[tauri::command]
pub(crate) async fn register_order(
    engine: State<'_, Engine>,
    input: RegisterOrderInput,
) -> Result<OrderDetail, CommandError> {
    engine.register_order(input).await
}

#[tauri::command]
pub(crate) async fn get_order(
    engine: State<'_, Engine>,
    input: GetOrderInput,
) -> Result<OrderDetail, CommandError> {
    engine.get_order(input).await
}

#[tauri::command]
pub(crate) async fn list_orders(
    engine: State<'_, Engine>,
    input: ListOrdersInput,
) -> Result<Vec<OrderSummary>, CommandError> {
    engine.list_orders(input).await
}

#[tauri::command]
pub(crate) async fn amend_order_evidence(
    engine: State<'_, Engine>,
    input: AmendOrderEvidenceInput,
) -> Result<OrderDetail, CommandError> {
    engine.amend_order_evidence(input).await
}

#[tauri::command]
pub(crate) async fn accept_order(
    engine: State<'_, Engine>,
    input: AcceptOrderInput,
) -> Result<OrderDetail, CommandError> {
    engine.accept_order(input).await
}

#[tauri::command]
pub(crate) async fn reject_order(
    engine: State<'_, Engine>,
    input: RejectOrderInput,
) -> Result<OrderDetail, CommandError> {
    engine.reject_order(input).await
}

#[tauri::command]
pub(crate) async fn place_clarification_hold(
    engine: State<'_, Engine>,
    input: PlaceClarificationHoldInput,
) -> Result<OrderDetail, CommandError> {
    engine.place_clarification_hold(input).await
}

#[tauri::command]
pub(crate) async fn resolve_clarification_hold(
    engine: State<'_, Engine>,
    input: ResolveClarificationHoldInput,
) -> Result<OrderDetail, CommandError> {
    engine.resolve_clarification_hold(input).await
}

#[tauri::command]
pub(crate) async fn create_billing_review_item(
    engine: State<'_, Engine>,
    input: CreateBillingReviewItemInput,
) -> Result<lis_engine::OrderBillingView, CommandError> {
    engine.create_billing_review_item(input).await
}

#[tauri::command]
pub(crate) async fn request_follow_on(
    engine: State<'_, Engine>,
    input: RequestFollowOnIpcInput,
) -> Result<OrderDetail, CommandError> {
    engine.request_follow_on(input.into()).await
}

#[tauri::command]
pub(crate) async fn decide_follow_on(
    engine: State<'_, Engine>,
    input: DecideFollowOnInput,
) -> Result<OrderDetail, CommandError> {
    engine.decide_follow_on(input).await
}

#[tauri::command]
pub(crate) async fn record_trigger_not_met(
    engine: State<'_, Engine>,
    input: RecordTriggerNotMetIpcInput,
) -> Result<OrderDetail, CommandError> {
    engine.record_trigger_not_met(input.into()).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{RecordTriggerNotMetIpcInput, RequestFollowOnIpcInput};

    #[test]
    fn public_follow_on_inputs_refuse_laboratory_result_sources() {
        let request = json!({
            "orderId": lis_engine::OrderId::generate(),
            "expectedRevision": 1,
            "kind": "reflex",
            "orderableVersionId": lis_core::Id::<lis_engine::OrderableVersion>::generate(),
            "parentItemId": lis_engine::OrderItemId::generate(),
            "authorityReference": null,
            "pocResultId": null,
            "triggerSource": "laboratory_result",
            "triggerRef": lis_engine::OrderId::generate(),
            "reason": "Synthetic reflex request"
        });
        assert!(serde_json::from_value::<RequestFollowOnIpcInput>(request).is_err());

        let not_met = json!({
            "parentItemId": lis_engine::OrderItemId::generate(),
            "orderableId": lis_engine::OrderableId::generate(),
            "triggerSource": "laboratory_result",
            "triggerRef": lis_engine::OrderId::generate(),
            "reason": "Synthetic condition assessment"
        });
        assert!(serde_json::from_value::<RecordTriggerNotMetIpcInput>(not_met).is_err());
    }
}

#[tauri::command]
pub(crate) async fn request_cancellation(
    engine: State<'_, Engine>,
    input: RequestCancellationInput,
) -> Result<OrderDetail, CommandError> {
    engine.request_cancellation(input).await
}

#[tauri::command]
pub(crate) async fn decide_cancellation(
    engine: State<'_, Engine>,
    input: DecideCancellationInput,
) -> Result<OrderDetail, CommandError> {
    engine.decide_cancellation(input).await
}

#[tauri::command]
pub(crate) async fn register_standing_order(
    engine: State<'_, Engine>,
    input: RegisterStandingOrderInput,
) -> Result<StandingOrderView, CommandError> {
    engine.register_standing_order(input).await
}

#[tauri::command]
pub(crate) async fn list_standing_orders(
    engine: State<'_, Engine>,
    input: ListStandingOrdersInput,
) -> Result<Vec<StandingOrderView>, CommandError> {
    engine.list_standing_orders(input).await
}
