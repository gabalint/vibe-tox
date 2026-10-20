//! Service derivation and billing work (REQ-BIL-001, REQ-BIL-005,
//! REQ-OPS-004). A derivation view carries subject identity, so each one
//! returned records an access event in the same transaction.

use lis_audit::AuditDetail;
use lis_billing::{
    ApproveServiceDerivation, BillingDerivationId, BillingError, BillingWorkItem, DerivationView,
    DeriveServices, ListBillingWork,
};
use lis_core::CommandError;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{
    APPROVE_SERVICE_DERIVATION, DERIVE_SERVICES, GET_SERVICE_DERIVATION, LIST_BILLING_WORK,
};
use crate::engine::Engine;
use crate::pipeline::access;

const RECORD_TYPE: &str = "billing_derivation";

/// Input of `get_service_derivation`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetServiceDerivationInput {
    pub derivation_id: BillingDerivationId,
}

impl Engine {
    /// Reconciles differing same-day financial arrangements against the exact input digest.
    pub async fn resolve_billing_context_conflict(
        &self,
        input: lis_billing::ResolveBillingContextConflictInput,
    ) -> Result<DerivationView, CommandError> {
        self.run(
            &super::RESOLVE_BILLING_CONTEXT_CONFLICT,
            |call| async move {
                self.write(&call, move |ctx| {
                    let id = lis_billing::resolve_billing_context_conflict(ctx, input)?;
                    derivation_read(ctx, super::RESOLVE_BILLING_CONTEXT_CONFLICT.action, id)
                })
                .await
            },
        )
        .await
    }

    /// `list_billing_work`: PHI-free billing work, oldest first.
    pub async fn list_billing_work(
        &self,
        input: ListBillingWork,
    ) -> Result<Vec<BillingWorkItem>, CommandError> {
        self.run(&LIST_BILLING_WORK, |call| async move {
            let filter = input.filter.as_str();
            self.audited_read(
                &call,
                move |conn| lis_billing::list_billing_work(conn, input),
                move |rows| {
                    access(LIST_BILLING_WORK.action)
                        .detail("filter", AuditDetail::code(filter))
                        .detail("returned", AuditDetail::count(rows.len()))
                },
            )
            .await
        })
        .await
    }

    /// `derive_services`: derives, or returns the unchanged newest derivation.
    pub async fn derive_services(
        &self,
        input: DeriveServices,
    ) -> Result<DerivationView, CommandError> {
        self.run(&DERIVE_SERVICES, |call| async move {
            self.write(&call, move |ctx| {
                let id = lis_billing::derive_services(ctx, input)?;
                derivation_read(ctx, DERIVE_SERVICES.action, id)
            })
            .await
        })
        .await
    }

    /// `get_service_derivation`: an audited read; staleness is recomputed.
    pub async fn get_service_derivation(
        &self,
        input: GetServiceDerivationInput,
    ) -> Result<DerivationView, CommandError> {
        self.run(&GET_SERVICE_DERIVATION, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_billing::get_service_derivation(conn, input.derivation_id),
                |view| {
                    access(GET_SERVICE_DERIVATION.action).record(RECORD_TYPE, view.derivation_id)
                },
            )
            .await
        })
        .await
    }

    /// `approve_service_derivation`: a current candidate becomes approved.
    pub async fn approve_service_derivation(
        &self,
        input: ApproveServiceDerivation,
    ) -> Result<DerivationView, CommandError> {
        self.run(&APPROVE_SERVICE_DERIVATION, |call| async move {
            self.write(&call, move |ctx| {
                let id = lis_billing::approve_service_derivation(ctx, input)?;
                derivation_read(ctx, APPROVE_SERVICE_DERIVATION.action, id)
            })
            .await
        })
        .await
    }
}

/// The derivation as it is now, with the access event of the read.
fn derivation_read(
    ctx: &CommandContext<'_>,
    action: &'static str,
    id: BillingDerivationId,
) -> Result<DerivationView, BillingError> {
    let view = lis_billing::get_service_derivation(ctx.tx(), id)?;
    ctx.audit(access(action).record(RECORD_TYPE, id))?;
    Ok(view)
}
