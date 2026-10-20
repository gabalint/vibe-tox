//! Financial intake reads and writes, separate from clinical authority (REQ-ORD-005).

use lis_core::{Capability, CommandError};
use lis_identity::{
    GetPatientBillingProfileInput, PatientBillingProfile, PatientBillingProfileView,
    SetPatientBillingProfileInput,
};
use lis_orders::{
    BillingReviewRecord, GetOrderInput, OrderBillingContext, OrderBillingView,
    SetOrderBillingContextInput, UpdateBillingReviewItemInput,
};

use crate::engine::Engine;
use crate::pipeline::access;
use crate::registry::{Access, CommandSpec, spec};

const INTAKE: Access = Access::EitherCapability(Capability::EnterOrders, Capability::Billing);

pub(crate) const GET_PATIENT_BILLING_PROFILE: CommandSpec = spec(
    "get_patient_billing_profile",
    "identity.get_billing_profile",
    INTAKE,
);
pub(crate) const SET_PATIENT_BILLING_PROFILE: CommandSpec = spec(
    "set_patient_billing_profile",
    "identity.set_billing_profile",
    INTAKE,
);
pub(crate) const GET_ORDER_BILLING: CommandSpec =
    spec("get_order_billing", "order.get_billing", INTAKE);
pub(crate) const SET_ORDER_BILLING_CONTEXT: CommandSpec = spec(
    "set_order_billing_context",
    "order.set_billing_context",
    INTAKE,
);
pub(crate) const UPDATE_BILLING_REVIEW_ITEM: CommandSpec = spec(
    "update_billing_review_item",
    "order.update_billing_review",
    INTAKE,
);

pub(crate) const LIST_BILLING_REVIEW_ITEMS: CommandSpec = spec(
    "list_billing_review_items",
    "order.list_billing_reviews",
    INTAKE,
);

impl Engine {
    /// Financial follow-up queue; patient details are read separately with access audit.
    pub async fn list_billing_review_items(
        &self,
        input: lis_orders::ListBillingReviewItemsInput,
    ) -> Result<Vec<BillingReviewRecord>, CommandError> {
        self.run(&LIST_BILLING_REVIEW_ITEMS, |call| async move {
            let principal = call.principal.clone();
            self.audited_read(
                &call,
                move |conn| lis_orders::list_billing_review_items(conn, &principal, input),
                |rows| {
                    access(LIST_BILLING_REVIEW_ITEMS.action)
                        .detail("returned", lis_audit::AuditDetail::count(rows.len()))
                },
            )
            .await
        })
        .await
    }

    /// Retrieves the selected historical or latest patient financial profile.
    pub async fn get_patient_billing_profile(
        &self,
        input: GetPatientBillingProfileInput,
    ) -> Result<PatientBillingProfileView, CommandError> {
        self.run(&GET_PATIENT_BILLING_PROFILE, |call| async move {
            let principal = call.principal.clone();
            self.audited_read(
                &call,
                move |conn| lis_identity::patient_billing_view(conn, &principal, input),
                |view| {
                    access(GET_PATIENT_BILLING_PROFILE.action).record("subject", view.subject_id)
                },
            )
            .await
        })
        .await
    }

    /// Appends a profile version without changing previous order selections.
    pub async fn set_patient_billing_profile(
        &self,
        input: SetPatientBillingProfileInput,
    ) -> Result<PatientBillingProfile, CommandError> {
        self.run(&SET_PATIENT_BILLING_PROFILE, |call| async move {
            self.write(&call, move |ctx| {
                let profile = lis_identity::set_patient_billing_profile(ctx, input)?;
                ctx.audit(
                    access(SET_PATIENT_BILLING_PROFILE.action)
                        .record("subject", profile.subject_id),
                )?;
                Ok::<_, lis_identity::IdentityError>(profile)
            })
            .await
        })
        .await
    }

    /// Reads an order's confirmed financial context and follow-up work.
    pub async fn get_order_billing(
        &self,
        input: GetOrderInput,
    ) -> Result<OrderBillingView, CommandError> {
        self.run(&GET_ORDER_BILLING, |call| async move {
            let principal = call.principal.clone();
            self.audited_read(
                &call,
                move |conn| lis_orders::order_billing_view(conn, &principal, input.order_id),
                |view| access(GET_ORDER_BILLING.action).record("order", view.order_id),
            )
            .await
        })
        .await
    }

    /// Records the selected billing arrangement and its exact supplied evidence.
    pub async fn set_order_billing_context(
        &self,
        input: SetOrderBillingContextInput,
    ) -> Result<OrderBillingContext, CommandError> {
        self.run(&SET_ORDER_BILLING_CONTEXT, |call| async move {
            self.write(&call, move |ctx| {
                let context = lis_orders::set_order_billing_context(ctx, input)?;
                ctx.audit(
                    access(SET_ORDER_BILLING_CONTEXT.action).record("order", context.order_id),
                )?;
                Ok::<_, lis_orders::OrderError>(context)
            })
            .await
        })
        .await
    }

    /// Assigns, resolves or reopens billing follow-up with retained evidence.
    pub async fn update_billing_review_item(
        &self,
        input: UpdateBillingReviewItemInput,
    ) -> Result<BillingReviewRecord, CommandError> {
        self.run(&UPDATE_BILLING_REVIEW_ITEM, |call| async move {
            self.write(&call, move |ctx| {
                let item = lis_orders::update_billing_review_item(ctx, input)?;
                ctx.audit(
                    access(UPDATE_BILLING_REVIEW_ITEM.action).record("order", item.order_id),
                )?;
                Ok::<_, lis_orders::OrderError>(item)
            })
            .await
        })
        .await
    }
}
