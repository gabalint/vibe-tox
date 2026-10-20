//! Report workspace, candidates, history, impact, delivery and destinations
//! (REQ-RPT-001/002/003/005/011/012/013, NFR-01, NFR-07), and closing
//! preservation incidents (REQ-OPS-004). Preparation,
//! release, withdrawal and export, which render or read files outside their
//! transactions, are in [`crate::report_rendering`].

use lis_core::{Actor, Capability, CommandError, Principal, UserId};
use lis_reporting::{
    AddDeliveryObligationInput, CandidateId, CandidateView, ClosePreservationIncidentInput,
    CreateDeliveryDestinationInput, DestinationView, ImpactView, ObligationView,
    PreservationIncidentView, RecordDeliveryAttemptInput, RecordImpactAssessmentInput,
    RedirectDeliveryObligationInput, ReplaceDeliveryDestinationInput, ReportDetail, ReportId,
    ReportRevisionId, ReportWorkspace, ReportingError, RetireDeliveryDestinationInput,
    RevisionDetail, VerifyDeliveryDestinationInput, actions,
};
use lis_specimens::SpecimenId;
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{Failure, access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const RELEASE: Access = Access::Capability(Capability::ReleaseReports);
const DELIVER: Access = Access::Capability(Capability::DeliverReports);
const CONFIGURE: Access = Access::Capability(Capability::ManageConfiguration);
const QUALITY: Access = Access::Capability(Capability::ManageQuality);

pub(crate) const GET_REPORT_WORKSPACE: CommandSpec =
    spec("get_report_workspace", "report.get_workspace", RELEASE);
pub(crate) const PREPARE_REPORT_CANDIDATE: CommandSpec = spec(
    "prepare_report_candidate",
    actions::PREPARE_CANDIDATE,
    RELEASE,
);
pub(crate) const GET_REPORT_CANDIDATE: CommandSpec =
    spec("get_report_candidate", "report.get_candidate", RELEASE);
pub(crate) const RELEASE_REPORT: CommandSpec = spec("release_report", actions::RELEASE, RELEASE);
pub(crate) const WITHDRAW_REPORT: CommandSpec = spec("withdraw_report", actions::WITHDRAW, RELEASE);
// Report history: `deliver_reports` or `manage_quality`, checked by
// `require_report_reader` (quality staff open the reports they assess).
pub(crate) const LIST_REPORTS: CommandSpec =
    spec("list_reports", "report.list_reports", Access::Session);
pub(crate) const GET_REPORT: CommandSpec = spec("get_report", "report.get_report", Access::Session);
pub(crate) const GET_REPORT_REVISION: CommandSpec = spec(
    "get_report_revision",
    "report.get_revision",
    Access::Session,
);
pub(crate) const EXPORT_REPORT_PDF: CommandSpec =
    spec("export_report_pdf", actions::EXPORT_PDF, DELIVER);
pub(crate) const RECORD_IMPACT_ASSESSMENT: CommandSpec =
    spec("record_impact_assessment", actions::ASSESS_IMPACT, QUALITY);
pub(crate) const CLOSE_PRESERVATION_INCIDENT: CommandSpec = spec(
    "close_preservation_incident",
    actions::CLOSE_PRESERVATION_INCIDENT,
    QUALITY,
);
pub(crate) const LIST_DELIVERY_OBLIGATIONS: CommandSpec = spec(
    "list_delivery_obligations",
    "report.list_obligations",
    DELIVER,
);
pub(crate) const RECORD_DELIVERY_ATTEMPT: CommandSpec =
    spec("record_delivery_attempt", actions::RECORD_DELIVERY, DELIVER);
pub(crate) const ADD_DELIVERY_OBLIGATION: CommandSpec =
    spec("add_delivery_obligation", actions::ADD_OBLIGATION, DELIVER);
pub(crate) const REDIRECT_DELIVERY_OBLIGATION: CommandSpec = spec(
    "redirect_delivery_obligation",
    actions::REDIRECT_OBLIGATION,
    DELIVER,
);
pub(crate) const LIST_DELIVERY_DESTINATIONS: CommandSpec = spec(
    "list_delivery_destinations",
    "report.list_destinations",
    Access::Session,
);
pub(crate) const CREATE_DELIVERY_DESTINATION: CommandSpec = spec(
    "create_delivery_destination",
    actions::CREATE_DESTINATION,
    CONFIGURE,
);
pub(crate) const VERIFY_DELIVERY_DESTINATION: CommandSpec = spec(
    "verify_delivery_destination",
    actions::VERIFY_DESTINATION,
    CONFIGURE,
);
pub(crate) const REPLACE_DELIVERY_DESTINATION: CommandSpec = spec(
    "replace_delivery_destination",
    actions::REPLACE_DESTINATION,
    CONFIGURE,
);
pub(crate) const RETIRE_DELIVERY_DESTINATION: CommandSpec = spec(
    "retire_delivery_destination",
    actions::RETIRE_DESTINATION,
    CONFIGURE,
);

domain_error!(
    ReportingError,
    ReportingError::field_errors,
    ReportingError::gate_reasons
);

/// Input of `get_report_workspace`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetReportWorkspaceInput {
    pub specimen_id: SpecimenId,
}

/// Input of `get_report_candidate`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetReportCandidateInput {
    pub candidate_id: CandidateId,
}

/// Input of `get_report`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetReportInput {
    pub report_id: ReportId,
}

/// Input of `get_report_revision`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetReportRevisionInput {
    pub revision_id: ReportRevisionId,
}

/// The signed-in user behind a session command.
pub(crate) fn user_of(principal: &Principal) -> Result<UserId, Failure> {
    match principal.actor() {
        Actor::User { user_id, .. } => Ok(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => Err(Failure::Internal(
            "report command without a user".to_owned(),
        )),
    }
}

impl Engine {
    /// `get_report_workspace`: what a candidate prepared now would contain.
    pub async fn get_report_workspace(
        &self,
        input: GetReportWorkspaceInput,
    ) -> Result<ReportWorkspace, CommandError> {
        self.run(&GET_REPORT_WORKSPACE, |call| async move {
            let viewer = user_of(&call.principal)?;
            let now = self.clock.now();
            let workspace = self
                .audited_report_read(
                    &call,
                    Capability::ReleaseReports,
                    move |conn| {
                        lis_reporting::report_workspace(conn, input.specimen_id, viewer, now)
                    },
                    move |_| {
                        access(GET_REPORT_WORKSPACE.action).record("specimen", input.specimen_id)
                    },
                )
                .await?;
            self.record_workspace_incident(&call, workspace.open_candidate_incident.clone())
                .await?;
            Ok(workspace)
        })
        .await
    }

    /// `get_report_candidate`: its exact printed strings, gates and staleness.
    pub async fn get_report_candidate(
        &self,
        input: GetReportCandidateInput,
    ) -> Result<CandidateView, CommandError> {
        self.run(&GET_REPORT_CANDIDATE, |call| async move {
            let now = self.clock.now();
            self.audited_report_read(
                &call,
                Capability::ReleaseReports,
                move |conn| lis_reporting::get_candidate(conn, input.candidate_id, now),
                move |_| {
                    access(GET_REPORT_CANDIDATE.action)
                        .record("report_candidate", input.candidate_id)
                },
            )
            .await
        })
        .await
    }

    /// `get_report`: revisions, obligations, exports, impact, withdrawal and
    /// incidents. Damaged stored content of the current revision is first
    /// recorded as an incident, which the report then lists (NFR-07).
    pub async fn get_report(&self, input: GetReportInput) -> Result<ReportDetail, CommandError> {
        self.run(&GET_REPORT, |call| async move {
            let capability = self.require_report_reader(&call).await?;
            self.record_damaged_content(&call, capability, input.report_id)
                .await?;
            let caller = call.principal.clone();
            self.audited_report_read(
                &call,
                capability,
                move |conn| lis_reporting::get_report(conn, input.report_id, &caller),
                move |_| access(GET_REPORT.action).record("report", input.report_id),
            )
            .await
        })
        .await
    }

    /// `get_report_revision`: exact content and provenance of a revision.
    pub async fn get_report_revision(
        &self,
        input: GetReportRevisionInput,
    ) -> Result<RevisionDetail, CommandError> {
        self.run(&GET_REPORT_REVISION, |call| async move {
            let capability = self.require_report_reader(&call).await?;
            self.audited_report_read(
                &call,
                capability,
                move |conn| lis_reporting::get_revision(conn, input.revision_id),
                move |_| {
                    access(GET_REPORT_REVISION.action).record("report_revision", input.revision_id)
                },
            )
            .await
        })
        .await
    }

    /// `record_impact_assessment` (`manage_quality`).
    pub async fn record_impact_assessment(
        &self,
        input: RecordImpactAssessmentInput,
    ) -> Result<ImpactView, CommandError> {
        self.run(&RECORD_IMPACT_ASSESSMENT, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::record_impact_assessment(ctx, input)
            })
            .await
        })
        .await
    }

    /// `close_preservation_incident` (`manage_quality`): a resolution and a
    /// reason; the incident stays readable.
    pub async fn close_preservation_incident(
        &self,
        input: ClosePreservationIncidentInput,
    ) -> Result<PreservationIncidentView, CommandError> {
        self.run(&CLOSE_PRESERVATION_INCIDENT, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::close_preservation_incident(ctx, input)
            })
            .await
        })
        .await
    }

    /// `record_delivery_attempt`: an attested attempt, never "delivered".
    pub async fn record_delivery_attempt(
        &self,
        input: RecordDeliveryAttemptInput,
    ) -> Result<ObligationView, CommandError> {
        self.run(&RECORD_DELIVERY_ATTEMPT, |call| async move {
            self.write(&call, move |ctx| {
                crate::report_rendering::ensure_outbound(ctx.tx())?;
                lis_reporting::record_delivery_attempt(ctx, input)
            })
            .await
        })
        .await
    }

    /// `add_delivery_obligation`.
    pub async fn add_delivery_obligation(
        &self,
        input: AddDeliveryObligationInput,
    ) -> Result<ObligationView, CommandError> {
        self.run(&ADD_DELIVERY_OBLIGATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::add_delivery_obligation(ctx, input)
            })
            .await
        })
        .await
    }

    /// `redirect_delivery_obligation`.
    pub async fn redirect_delivery_obligation(
        &self,
        input: RedirectDeliveryObligationInput,
    ) -> Result<ObligationView, CommandError> {
        self.run(&REDIRECT_DELIVERY_OBLIGATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::redirect_delivery_obligation(ctx, input)
            })
            .await
        })
        .await
    }

    /// `create_delivery_destination`: born unverified.
    pub async fn create_delivery_destination(
        &self,
        input: CreateDeliveryDestinationInput,
    ) -> Result<DestinationView, CommandError> {
        self.run(&CREATE_DELIVERY_DESTINATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::create_delivery_destination(ctx, input)
            })
            .await
        })
        .await
    }

    /// `verify_delivery_destination`: by a second person, or by its creator where the
    /// laboratory's separation-of-duties policy allows it (ADR 0015).
    pub async fn verify_delivery_destination(
        &self,
        input: VerifyDeliveryDestinationInput,
    ) -> Result<DestinationView, CommandError> {
        self.run(&VERIFY_DELIVERY_DESTINATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::verify_delivery_destination(ctx, input)
            })
            .await
        })
        .await
    }

    /// `replace_delivery_destination`: retire and create an unverified replacement.
    pub async fn replace_delivery_destination(
        &self,
        input: ReplaceDeliveryDestinationInput,
    ) -> Result<DestinationView, CommandError> {
        self.run(&REPLACE_DELIVERY_DESTINATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::replace_delivery_destination(ctx, input)
            })
            .await
        })
        .await
    }

    /// `retire_delivery_destination`.
    pub async fn retire_delivery_destination(
        &self,
        input: RetireDeliveryDestinationInput,
    ) -> Result<DestinationView, CommandError> {
        self.run(&RETIRE_DELIVERY_DESTINATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_reporting::retire_delivery_destination(ctx, input)
            })
            .await
        })
        .await
    }
}

/// Exact delivery task lookup, independent of queue page/filter.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetDeliveryObligationInput {
    pub obligation_id: lis_reporting::ObligationId,
}
pub(crate) const GET_DELIVERY_OBLIGATION: CommandSpec =
    spec("get_delivery_obligation", "report.get_obligation", DELIVER);
impl Engine {
    pub async fn get_delivery_obligation(
        &self,
        input: GetDeliveryObligationInput,
    ) -> Result<ObligationView, CommandError> {
        self.run(&GET_DELIVERY_OBLIGATION, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_reporting::get_delivery_obligation(conn, input.obligation_id),
                move |_| {
                    access(GET_DELIVERY_OBLIGATION.action)
                        .record("delivery_obligation", input.obligation_id)
                },
            )
            .await
        })
        .await
    }
}
