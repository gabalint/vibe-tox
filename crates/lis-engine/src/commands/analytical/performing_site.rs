//! Attributable recovery of an unpinned performing site (RPT-05).

use lis_analytical::{RecordRunPerformingSiteInput, RunDetail};
use lis_core::CommandError;

use super::{MANAGE_QUALITY, audit_run_response};
use crate::Engine;
use crate::registry::{CommandSpec, spec};

pub(crate) const RECORD_RUN_PERFORMING_SITE: CommandSpec = spec(
    "record_run_performing_site",
    "analytical.record_run_performing_site",
    MANAGE_QUALITY,
);

impl Engine {
    /// Record source-backed performing-site evidence without changing observations.
    pub async fn record_run_performing_site(
        &self,
        input: RecordRunPerformingSiteInput,
    ) -> Result<RunDetail, CommandError> {
        self.run(&RECORD_RUN_PERFORMING_SITE, |call| async move {
            self.write(&call, move |ctx| {
                let detail = lis_analytical::record_run_performing_site(ctx, input)?;
                audit_run_response(ctx, RECORD_RUN_PERFORMING_SITE.action, detail)
            })
            .await
        })
        .await
    }
}
