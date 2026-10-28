//! The Report templates page (REQ-RPT-007): the laboratory's report layout
//! choice, drafted by configuration staff and approved or rejected by quality
//! staff, and previews of each layout on synthetic sample data.

use lis_core::{Capability, CommandError};
use lis_reporting::layouts::ReportLayout;
use lis_reporting::{
    ApproveReportLayoutInput, DraftReportLayoutInput, RejectReportLayoutInput, ReportLayoutsView,
    ReportingError, actions,
};
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::Failure;
use crate::registry::{Access, CommandSpec, spec};

const CONFIGURE: Access = Access::Capability(Capability::ManageConfiguration);
const QUALITY: Access = Access::Capability(Capability::ManageQuality);
const CONFIGURE_OR_QUALITY: Access =
    Access::EitherCapability(Capability::ManageConfiguration, Capability::ManageQuality);

pub(crate) const GET_REPORT_LAYOUTS: CommandSpec = spec(
    "get_report_layouts",
    "report.get_layouts",
    CONFIGURE_OR_QUALITY,
);
pub(crate) const PREVIEW_REPORT_LAYOUT: CommandSpec = spec(
    "preview_report_layout",
    "report.preview_layout",
    CONFIGURE_OR_QUALITY,
);
pub(crate) const DRAFT_REPORT_LAYOUT: CommandSpec =
    spec("draft_report_layout", actions::DRAFT_LAYOUT, CONFIGURE);
pub(crate) const APPROVE_REPORT_LAYOUT: CommandSpec =
    spec("approve_report_layout", actions::APPROVE_LAYOUT, QUALITY);
pub(crate) const REJECT_REPORT_LAYOUT: CommandSpec =
    spec("reject_report_layout", actions::REJECT_LAYOUT, QUALITY);

/// Input of `preview_report_layout`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PreviewReportLayoutInput {
    pub layout: ReportLayout,
}

impl Engine {
    /// `get_report_layouts`: the layouts, the one in force, the draft and the
    /// version history. Configuration, no PHI.
    pub async fn get_report_layouts(&self) -> Result<ReportLayoutsView, CommandError> {
        self.run(&GET_REPORT_LAYOUTS, |call| async move {
            let caller = call.principal.clone();
            self.read(&call, move |conn| {
                lis_reporting::report_layouts(conn, &caller)
            })
            .await
        })
        .await
    }

    /// `preview_report_layout`: the layout rendered on the built-in synthetic
    /// sample, marked "PREVIEW – NOT A REPORT", for the desktop shell to show in
    /// a window of its own. No laboratory data is read, and nothing is stored.
    pub async fn preview_report_layout(
        &self,
        input: PreviewReportLayoutInput,
    ) -> Result<Vec<u8>, CommandError> {
        self.run(&PREVIEW_REPORT_LAYOUT, |call| async move {
            self.read(&call, |_| Ok::<_, ReportingError>(())).await?;
            let issued_at = self
                .clock
                .now()
                .to_string()
                .parse()
                .map_err(|_| Failure::Internal("layout preview time".to_owned()))?;
            let template = input.layout.template();
            let pdf = tokio::task::spawn_blocking(move || {
                lis_report::preview::render_preview(&template, issued_at)
            })
            .await
            .map_err(|_| Failure::Internal("layout preview task stopped".to_owned()))?
            .map_err(|error| Failure::Internal(format!("layout preview not rendered: {error}")))?;
            Ok(pdf.bytes)
        })
        .await
    }

    /// `draft_report_layout`: affects no report until approved.
    pub async fn draft_report_layout(
        &self,
        input: DraftReportLayoutInput,
    ) -> Result<ReportLayoutsView, CommandError> {
        self.run(&DRAFT_REPORT_LAYOUT, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::draft_report_layout(ctx, input)
            })
            .await
        })
        .await
    }

    /// `approve_report_layout`: by quality staff; by the draft's author only
    /// where the laboratory's separation-of-duties policy allows it (ADR 0015).
    pub async fn approve_report_layout(
        &self,
        input: ApproveReportLayoutInput,
    ) -> Result<ReportLayoutsView, CommandError> {
        self.run(&APPROVE_REPORT_LAYOUT, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::approve_report_layout(ctx, input)
            })
            .await
        })
        .await
    }

    /// `reject_report_layout`: with a reason; the active layout stays in force.
    pub async fn reject_report_layout(
        &self,
        input: RejectReportLayoutInput,
    ) -> Result<ReportLayoutsView, CommandError> {
        self.run(&REJECT_REPORT_LAYOUT, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::reject_report_layout(ctx, input)
            })
            .await
        })
        .await
    }
}
