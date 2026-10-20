//! Audited order-to-accession progress for intake and receipt (REQ-ORD-001, REQ-SPC-001/002).

use lis_core::{Capability, CommandError, Principal};
use lis_orders::GetOrderInput;
use lis_specimens::{OrderAccessionContext, actions};

use crate::engine::Engine;
use crate::pipeline::access;
use crate::registry::{Access, CommandSpec, spec};

pub(crate) const GET_ORDER_ACCESSION_CONTEXT: CommandSpec = spec(
    "get_order_accession_context",
    actions::VIEW_ORDER_ACCESSIONS,
    Access::EitherCapability(Capability::EnterOrders, Capability::Accession),
);

impl Engine {
    /// Audited membership, receipt progress and actionable remaining material for an order.
    pub async fn get_order_accession_context(
        &self,
        input: GetOrderInput,
    ) -> Result<OrderAccessionContext, CommandError> {
        self.run(&GET_ORDER_ACCESSION_CONTEXT, |call| async move {
            let principal = call.principal.clone();
            self.audited_read(
                &call,
                move |conn| {
                    let mut context =
                        lis_specimens::get_order_accession_context(conn, input.order_id)?;
                    restrict_actions(&mut context, &principal);
                    Ok::<_, lis_specimens::SpecimenError>(context)
                },
                |context| {
                    access(GET_ORDER_ACCESSION_CONTEXT.action).record("order", context.order_id)
                },
            )
            .await
        })
        .await
    }
}

fn restrict_actions(context: &mut OrderAccessionContext, principal: &Principal) {
    if principal.require(Capability::Accession).is_err() {
        context.allowed_actions.clear();
    }
}

#[cfg(test)]
mod tests {
    use lis_core::{SessionId, UserId, WorkstationId};

    use super::*;

    #[test]
    fn each_intake_capability_allows_the_read_but_unrelated_capabilities_do_not() {
        for (capabilities, allowed) in [
            (vec![Capability::EnterOrders], true),
            (vec![Capability::Accession], true),
            (vec![Capability::EnterOrders, Capability::Accession], true),
            (vec![Capability::ManageConfiguration], false),
            (Vec::new(), false),
        ] {
            let principal = Principal::user(
                UserId::generate(),
                SessionId::generate(),
                WorkstationId::generate(),
                capabilities,
            );
            assert_eq!(
                GET_ORDER_ACCESSION_CONTEXT
                    .access
                    .require(&principal)
                    .is_ok(),
                allowed
            );
            let mut context = OrderAccessionContext {
                order_id: lis_orders::OrderId::generate(),
                items: Vec::new(),
                accessions: Vec::new(),
                allowed_actions: vec!["create_accession".to_owned()],
            };
            restrict_actions(&mut context, &principal);
            assert_eq!(
                !context.allowed_actions.is_empty(),
                principal.require(Capability::Accession).is_ok()
            );
        }
    }
}
