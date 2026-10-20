//! Order entry, supplied evidence, standing orders, clarification holds and
//! request/decision workflows (REQ-ORD-001–003, 005–007; INV-13).

use lis_audit::AuditDetail;
use lis_core::{Capability, CommandError};
use lis_orders::{
    AcceptOrderInput, AmendOrderEvidenceInput, CreateBillingReviewItemInput,
    DecideCancellationInput, DecideFollowOnInput, GetOrderInput, ListOrdersInput,
    ListStandingOrdersInput, OrderDetail, OrderError, OrderSummary, PlaceClarificationHoldInput,
    RecordTriggerNotMetInput, RegisterOrderInput, RegisterStandingOrderInput, RejectOrderInput,
    RequestCancellationInput, RequestFollowOnInput, ResolveClarificationHoldInput,
    StandingOrderView,
};
use lis_store::CommandContext;

use crate::engine::Engine;
use crate::pipeline::{access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const ENTER_ORDERS: Access = Access::Capability(Capability::EnterOrders);

pub(crate) const REGISTER_ORDER: CommandSpec =
    spec("register_order", "order.register", ENTER_ORDERS);
pub(crate) const GET_ORDER: CommandSpec = spec("get_order", "order.get", ENTER_ORDERS);
pub(crate) const LIST_ORDERS: CommandSpec = spec("list_orders", "order.list", ENTER_ORDERS);
pub(crate) const AMEND_ORDER_EVIDENCE: CommandSpec =
    spec("amend_order_evidence", "order.amend_evidence", ENTER_ORDERS);
pub(crate) const ACCEPT_ORDER: CommandSpec = spec("accept_order", "order.accept", ENTER_ORDERS);
pub(crate) const REJECT_ORDER: CommandSpec = spec("reject_order", "order.reject", ENTER_ORDERS);
pub(crate) const PLACE_CLARIFICATION_HOLD: CommandSpec =
    spec("place_clarification_hold", "order.hold", ENTER_ORDERS);
pub(crate) const RESOLVE_CLARIFICATION_HOLD: CommandSpec = spec(
    "resolve_clarification_hold",
    "order.resolve_hold",
    ENTER_ORDERS,
);
pub(crate) const CREATE_BILLING_REVIEW_ITEM: CommandSpec = spec(
    "create_billing_review_item",
    "order.billing_review",
    Access::EitherCapability(Capability::EnterOrders, Capability::Billing),
);
pub(crate) const REQUEST_FOLLOW_ON: CommandSpec =
    spec("request_follow_on", "order.request_follow_on", ENTER_ORDERS);
pub(crate) const DECIDE_FOLLOW_ON: CommandSpec =
    spec("decide_follow_on", "order.decide_follow_on", ENTER_ORDERS);
pub(crate) const RECORD_TRIGGER_NOT_MET: CommandSpec = spec(
    "record_trigger_not_met",
    "order.trigger_not_met",
    ENTER_ORDERS,
);
pub(crate) const REQUEST_CANCELLATION: CommandSpec =
    spec("request_cancellation", "order.request_cancel", ENTER_ORDERS);
pub(crate) const DECIDE_CANCELLATION: CommandSpec =
    spec("decide_cancellation", "order.decide_cancel", ENTER_ORDERS);
pub(crate) const REGISTER_STANDING_ORDER: CommandSpec = spec(
    "register_standing_order",
    "order.register_standing_order",
    ENTER_ORDERS,
);
pub(crate) const LIST_STANDING_ORDERS: CommandSpec = spec(
    "list_standing_orders",
    "order.list_standing_orders",
    ENTER_ORDERS,
);

domain_error!(OrderError, |error: &OrderError| error
    .field_errors()
    .to_vec());

fn audit_order_response(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    detail: OrderDetail,
) -> Result<OrderDetail, OrderError> {
    ctx.audit(access(action).record("order", detail.order_id))?;
    Ok(detail)
}

fn audit_standing_order_response(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    detail: StandingOrderView,
) -> Result<StandingOrderView, OrderError> {
    ctx.audit(access(action).record("standing_order", detail.standing_order_id))?;
    Ok(detail)
}

impl Engine {
    /// `register_order`.
    pub async fn register_order(
        &self,
        input: RegisterOrderInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&REGISTER_ORDER, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::register_order(ctx, input)?;
                audit_order_response(ctx, REGISTER_ORDER.action, detail)
            })
            .await
        })
        .await
    }

    /// `get_order` (audited sensitive read).
    pub async fn get_order(&self, input: GetOrderInput) -> Result<OrderDetail, CommandError> {
        self.run(&GET_ORDER, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_orders::get_order(conn, input.order_id),
                |order| access(GET_ORDER.action).record("order", order.order_id),
            )
            .await
        })
        .await
    }

    /// `list_orders` (audited because it identifies patient orders).
    pub async fn list_orders(
        &self,
        input: ListOrdersInput,
    ) -> Result<Vec<OrderSummary>, CommandError> {
        let filter = input.filter;
        self.run(&LIST_ORDERS, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_orders::list_orders(conn, &input),
                move |orders| {
                    access(LIST_ORDERS.action)
                        .detail("filter", AuditDetail::code(filter.as_str()))
                        .detail("returned", AuditDetail::count(orders.len()))
                },
            )
            .await
        })
        .await
    }

    /// `amend_order_evidence`.
    pub async fn amend_order_evidence(
        &self,
        input: AmendOrderEvidenceInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&AMEND_ORDER_EVIDENCE, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::amend_order_evidence(ctx, input)?;
                audit_order_response(ctx, AMEND_ORDER_EVIDENCE.action, detail)
            })
            .await
        })
        .await
    }

    /// `accept_order`.
    pub async fn accept_order(&self, input: AcceptOrderInput) -> Result<OrderDetail, CommandError> {
        self.run(&ACCEPT_ORDER, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::accept_order(ctx, input)?;
                audit_order_response(ctx, ACCEPT_ORDER.action, detail)
            })
            .await
        })
        .await
    }

    /// `reject_order`.
    pub async fn reject_order(&self, input: RejectOrderInput) -> Result<OrderDetail, CommandError> {
        self.run(&REJECT_ORDER, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::reject_order(ctx, input)?;
                audit_order_response(ctx, REJECT_ORDER.action, detail)
            })
            .await
        })
        .await
    }

    /// `place_clarification_hold`.
    pub async fn place_clarification_hold(
        &self,
        input: PlaceClarificationHoldInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&PLACE_CLARIFICATION_HOLD, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::place_clarification_hold(ctx, input)?;
                audit_order_response(ctx, PLACE_CLARIFICATION_HOLD.action, detail)
            })
            .await
        })
        .await
    }

    /// `resolve_clarification_hold`.
    pub async fn resolve_clarification_hold(
        &self,
        input: ResolveClarificationHoldInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&RESOLVE_CLARIFICATION_HOLD, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::resolve_clarification_hold(ctx, input)?;
                audit_order_response(ctx, RESOLVE_CLARIFICATION_HOLD.action, detail)
            })
            .await
        })
        .await
    }

    /// `create_billing_review_item`.
    pub async fn create_billing_review_item(
        &self,
        input: CreateBillingReviewItemInput,
    ) -> Result<lis_orders::OrderBillingView, CommandError> {
        self.run(&CREATE_BILLING_REVIEW_ITEM, |call| async move {
            self.write(&call, move |ctx| {
                let order_id = input.order_id;
                lis_orders::create_billing_review_item(ctx, input)?;
                let view = lis_orders::order_billing_view(ctx.tx(), ctx.principal(), order_id)?;
                ctx.audit(access(CREATE_BILLING_REVIEW_ITEM.action).record("order", order_id))?;
                Ok::<_, lis_orders::OrderError>(view)
            })
            .await
        })
        .await
    }

    /// `request_follow_on`.
    pub async fn request_follow_on(
        &self,
        input: RequestFollowOnInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&REQUEST_FOLLOW_ON, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::request_follow_on(ctx, input)?;
                audit_order_response(ctx, REQUEST_FOLLOW_ON.action, detail)
            })
            .await
        })
        .await
    }

    /// `decide_follow_on`.
    pub async fn decide_follow_on(
        &self,
        input: DecideFollowOnInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&DECIDE_FOLLOW_ON, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::decide_follow_on(ctx, input)?;
                audit_order_response(ctx, DECIDE_FOLLOW_ON.action, detail)
            })
            .await
        })
        .await
    }

    /// `record_trigger_not_met`.
    pub async fn record_trigger_not_met(
        &self,
        input: RecordTriggerNotMetInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&RECORD_TRIGGER_NOT_MET, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::record_trigger_not_met(ctx, input)?;
                audit_order_response(ctx, RECORD_TRIGGER_NOT_MET.action, detail)
            })
            .await
        })
        .await
    }

    /// `request_cancellation`.
    pub async fn request_cancellation(
        &self,
        input: RequestCancellationInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&REQUEST_CANCELLATION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::request_cancellation(ctx, input)?;
                audit_order_response(ctx, REQUEST_CANCELLATION.action, detail)
            })
            .await
        })
        .await
    }

    /// `decide_cancellation`.
    pub async fn decide_cancellation(
        &self,
        input: DecideCancellationInput,
    ) -> Result<OrderDetail, CommandError> {
        self.run(&DECIDE_CANCELLATION, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::decide_cancellation(ctx, input)?;
                audit_order_response(ctx, DECIDE_CANCELLATION.action, detail)
            })
            .await
        })
        .await
    }

    /// `register_standing_order`.
    pub async fn register_standing_order(
        &self,
        input: RegisterStandingOrderInput,
    ) -> Result<StandingOrderView, CommandError> {
        self.run(&REGISTER_STANDING_ORDER, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_orders::register_standing_order(ctx, input)?;
                audit_standing_order_response(ctx, REGISTER_STANDING_ORDER.action, detail)
            })
            .await
        })
        .await
    }

    /// `list_standing_orders` (audited because the subject is identified).
    pub async fn list_standing_orders(
        &self,
        input: ListStandingOrdersInput,
    ) -> Result<Vec<StandingOrderView>, CommandError> {
        self.run(&LIST_STANDING_ORDERS, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_orders::list_standing_orders(conn, input.subject_id),
                move |orders| {
                    access(LIST_STANDING_ORDERS.action)
                        .record("subject", input.subject_id)
                        .detail("returned", AuditDetail::count(orders.len()))
                },
            )
            .await
        })
        .await
    }
}
