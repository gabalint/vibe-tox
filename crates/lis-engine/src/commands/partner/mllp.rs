//! Testing an MLLP partner's interface as entered in the connection dialog (ticket #4555
//! step 4). Network work never runs in a transaction.

use lis_core::CommandError;
use lis_interfaces::partner::MllpPartner;
use serde::Deserialize;
use ts_rs::TS;

use super::TEST_PARTNER_MLLP;
use super::folders::FolderTestResult;
use crate::engine::Engine;
use crate::partner_delivery::{ProbeProblem, mllp};

/// Input of `test_partner_mllp`: the partner's interface as entered.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TestPartnerMllpInput {
    pub partner: MllpPartner,
}

impl Engine {
    /// `test_partner_mllp`: whether the partner's interface on the lab network takes a
    /// connection. Sends no message and saves nothing.
    pub async fn test_partner_mllp(
        &self,
        input: TestPartnerMllpInput,
    ) -> Result<FolderTestResult, CommandError> {
        self.run(&TEST_PARTNER_MLLP, |call| async move {
            self.authorize(&call).await?;
            let partner = lis_interfaces::partner::valid_mllp_partner(input.partner)?;
            let tested = mllp::probe(&partner).await.map_err(ProbeProblem::Mllp);
            Ok(FolderTestResult::probed(tested))
        })
        .await
    }
}
