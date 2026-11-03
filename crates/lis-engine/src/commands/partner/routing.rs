//! Routing a new billing handoff to its connections, in the transaction that creates it,
//! so a committed handoff always has its message (REQ-BIL-002, REQ-RPT-003).

use lis_billing::{BillingError, HandoffParties, HandoffView, Party};
use lis_core::Denied;
use lis_interfaces::partner::{
    NewPartnerMessage, PartnerError, PartnerMessageType, RoutingFacts, RoutingValue,
};
use lis_store::{CommandContext, StoreError};

use crate::pipeline::{Failure, ModuleError};

/// Creating a handoff and routing it share one transaction.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HandoffError {
    #[error(transparent)]
    Billing(#[from] BillingError),
    #[error(transparent)]
    Partner(#[from] PartnerError),
}

impl From<StoreError> for HandoffError {
    fn from(error: StoreError) -> Self {
        Self::Billing(error.into())
    }
}

impl ModuleError for HandoffError {
    fn denied(&self) -> Option<Denied> {
        match self {
            Self::Billing(error) => ModuleError::denied(error),
            Self::Partner(error) => ModuleError::denied(error),
        }
    }
}

impl From<HandoffError> for Failure {
    fn from(error: HandoffError) -> Self {
        match error {
            HandoffError::Billing(error) => error.into(),
            HandoffError::Partner(error) => error.into(),
        }
    }
}

/// Makes the handoff's message for every Billing JSON connection whose rules match, or
/// puts it in Unrouted messages. Without any Billing JSON connection nothing is made.
pub(crate) fn route_billing_handoff(
    ctx: &mut CommandContext<'_>,
    handoff: &HandoffView,
) -> Result<(), HandoffError> {
    if !lis_interfaces::partner::carries(ctx.tx(), PartnerMessageType::BillingJson)? {
        return Ok(());
    }
    let parties = lis_billing::handoff_parties(ctx.tx(), handoff.handoff_id)?;
    lis_interfaces::partner::route_partner_message(
        ctx,
        NewPartnerMessage {
            message_type: PartnerMessageType::BillingJson,
            source_id: handoff.handoff_id.to_string(),
            source_number: handoff.handoff_no,
            artifact_sha256: handoff.artifact_sha256.clone(),
            facts: facts(parties),
        },
    )?;
    Ok(())
}

fn facts(parties: HandoffParties) -> RoutingFacts {
    let values = |parties: Vec<Party>| {
        parties
            .into_iter()
            .map(|(value, label)| RoutingValue { value, label })
            .collect()
    };
    RoutingFacts {
        clients: values(parties.clients),
        practitioners: values(parties.practitioners),
        locations: values(parties.locations),
        tests: values(parties.tests),
    }
}
