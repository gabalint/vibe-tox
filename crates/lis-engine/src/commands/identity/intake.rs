//! Retained intake with incomplete identity, separate from testable patient records.
use super::ENTER_ORDERS;
use crate::pipeline::access;
use crate::{
    Engine,
    registry::{CommandSpec, spec},
};
use lis_core::CommandError;
use lis_identity::{
    IntakeDeferralView, ListIntakeDeferralsInput, RecordIntakeDeferralInput,
    ResolveIntakeDeferralInput,
};
pub(crate) const RECORD_INTAKE_DEFERRAL: CommandSpec = spec(
    "record_intake_deferral",
    "identity.record_intake_deferral",
    ENTER_ORDERS,
);
pub(crate) const RESOLVE_INTAKE_DEFERRAL: CommandSpec = spec(
    "resolve_intake_deferral",
    "identity.resolve_intake_deferral",
    ENTER_ORDERS,
);
pub(crate) const LIST_INTAKE_DEFERRALS: CommandSpec = spec(
    "list_intake_deferrals",
    "identity.list_intake_deferrals",
    ENTER_ORDERS,
);
impl Engine {
    pub async fn record_intake_deferral(
        &self,
        input: RecordIntakeDeferralInput,
    ) -> Result<IntakeDeferralView, CommandError> {
        self.run(&RECORD_INTAKE_DEFERRAL, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::record_intake_deferral(ctx, input)
            })
            .await
        })
        .await
    }
    pub async fn resolve_intake_deferral(
        &self,
        input: ResolveIntakeDeferralInput,
    ) -> Result<IntakeDeferralView, CommandError> {
        self.run(&RESOLVE_INTAKE_DEFERRAL, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::resolve_intake_deferral(ctx, input)
            })
            .await
        })
        .await
    }
    pub async fn list_intake_deferrals(
        &self,
        input: ListIntakeDeferralsInput,
    ) -> Result<Vec<IntakeDeferralView>, CommandError> {
        self.run(&LIST_INTAKE_DEFERRALS, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_identity::list_intake_deferrals(conn, input),
                |_| access(LIST_INTAKE_DEFERRALS.action),
            )
            .await
        })
        .await
    }
}
