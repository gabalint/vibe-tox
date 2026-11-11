//! Testing an MLLP partner's interface, or an MLLP listener, as entered in the connection
//! dialog (ticket #4555 step 4). Network work never runs in a transaction.

use lis_core::{CommandError, ErrorCode};
use lis_interfaces::partner::{MllpListener, MllpPartner, PartnerConnectionId};
use serde::Deserialize;
use ts_rs::TS;

use super::TEST_PARTNER_MLLP;
use super::folders::FolderTestResult;
use crate::engine::Engine;
use crate::partner_delivery::{ProbeProblem, mllp};
use crate::partner_listener::bind;
use crate::pipeline::Failure;

/// Input of `test_partner_mllp`: the partner's interface, or the listener, as entered.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TestPartnerMllpInput {
    /// MLLP – LIS connects: the partner's interface.
    #[serde(default)]
    #[ts(optional)]
    pub partner: Option<MllpPartner>,
    /// MLLP – LIS listens: the address and port to listen on.
    #[serde(default)]
    #[ts(optional)]
    pub listener: Option<MllpListener>,
    /// The connection being changed: its own listener may already listen there.
    #[serde(default)]
    #[ts(optional)]
    pub connection_id: Option<PartnerConnectionId>,
}

impl Engine {
    /// `test_partner_mllp`: whether the partner's interface on the lab network takes a
    /// connection, or whether the LIS can listen on the address and port: free, or already
    /// the connection's own listener. Sends no message and saves nothing.
    pub async fn test_partner_mllp(
        &self,
        input: TestPartnerMllpInput,
    ) -> Result<FolderTestResult, CommandError> {
        self.run(&TEST_PARTNER_MLLP, |call| async move {
            self.authorize(&call).await?;
            let tested = match (input.partner, input.listener) {
                (Some(partner), None) => {
                    let partner = lis_interfaces::partner::valid_mllp_partner(partner)?;
                    mllp::probe(&partner).await.map_err(ProbeProblem::Mllp)
                }
                (None, Some(listener)) => {
                    let listener = lis_interfaces::partner::valid_mllp_listener(listener)?;
                    self.listener_test(input.connection_id, &listener)
                }
                _ => {
                    return Err(Failure::refused(
                        ErrorCode::Validation,
                        "Enter the partner's interface or the listener.",
                    ));
                }
            };
            Ok(FolderTestResult::probed(tested))
        })
        .await
    }

    /// Whether the LIS can listen on the listener's address and port: they are free, or
    /// the connection's own listener already listens there.
    pub(super) fn listener_test(
        &self,
        connection_id: Option<PartnerConnectionId>,
        listener: &MllpListener,
    ) -> Result<(), ProbeProblem> {
        let own = connection_id.is_some_and(|id| self.partner_listeners.listens_on(id, listener));
        if own {
            return Ok(());
        }
        bind(listener).map(drop).map_err(ProbeProblem::Listener)
    }
}
