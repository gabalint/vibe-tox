//! Read models returned to the engine and the UI (brief §7). Printed content
//! is `ToxReportData`, exactly what the PDF shows; labels come from the
//! engine only.

use lis_core::{GateReason, UserId, UtcTimestamp};
use lis_identity::{ClientId, ClientLocationId, SubjectSummary};
use lis_orders::OrderComponentId;
use lis_report::tox_v2::ToxReportData;
use lis_specimens::SpecimenId;
use serde::Serialize;
use ts_rs::TS;

use crate::model::{
    CandidateId, CandidateState, DeliveryAttemptId, DeliveryChannel, DeliveryEvidence,
    DeliveryFailureCode, DestinationId, ExportId, ExportPurpose, ImpactAssessmentId,
    ImpactDecision, LineDisposition, ObligationId, ObligationKind, ObligationOrigin,
    ObligationStatus, ReportCompleteness, ReportId, ReportRelationship, ReportRevisionId,
    RevisionAvailability,
};
use crate::queries::destinations::DestinationState;

mod preservation;
pub use preservation::{IncidentClosureView, PreservationIncidentView};

/// A user named on a report record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportUserView {
    pub user_id: UserId,
    pub display_name: String,
}

/// One line's disposition with what holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DispositionView {
    /// `None` for a reflex trigger line.
    pub component_id: Option<OrderComponentId>,
    pub name: String,
    pub disposition: LineDisposition,
    pub gates: Vec<GateReason>,
}

/// One released revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RevisionSummary {
    pub revision_id: ReportRevisionId,
    #[ts(type = "number")]
    pub revision_no: i64,
    pub relationship: ReportRelationship,
    pub completeness: ReportCompleteness,
    pub availability: RevisionAvailability,
    pub released_at: UtcTimestamp,
    pub released_by: ReportUserView,
    pub amendment_reason: Option<String>,
    pub artifact_sha256: String,
}

/// A candidate as the releaser reviews it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CandidateView {
    pub candidate_id: CandidateId,
    #[ts(type = "number")]
    pub revision: i64,
    pub state: CandidateState,
    pub report_id: ReportId,
    pub report_number: String,
    #[ts(type = "number")]
    pub proposed_revision_no: i64,
    pub relationship: ReportRelationship,
    pub completeness: ReportCompleteness,
    #[ts(type = "number | null")]
    pub supersedes_revision_no: Option<i64>,
    pub amendment_reason: Option<String>,
    pub content_sha256: String,
    pub artifact_sha256: Option<String>,
    pub prepared_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub signatory: ReportUserView,
    pub content: ToxReportData,
    pub dispositions: Vec<DispositionView>,
    /// Gates as of now (rows 8–12).
    pub gates: Vec<GateReason>,
    /// The content rebuilt now differs from the candidate's.
    pub stale: bool,
    pub stale_reasons: Vec<GateReason>,
    pub allowed_actions: Vec<String>,
}

/// The report of a workspace, once it exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceReport {
    pub report_id: ReportId,
    pub report_number: String,
    pub current_revision: Option<RevisionSummary>,
    /// `None` until a revision is released.
    pub availability: Option<RevisionAvailability>,
}

/// What a candidate prepared now would contain: a hint only (brief §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportPreview {
    /// `None` while a gate prevents building the content.
    pub content: Option<ToxReportData>,
    pub dispositions: Vec<DispositionView>,
    pub completeness: Option<ReportCompleteness>,
    pub relationship: Option<ReportRelationship>,
    pub gates: Vec<GateReason>,
}

/// The release workspace of one specimen.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportWorkspace {
    /// The unreadable unissued candidate retained for preservation investigation.
    pub unavailable_candidate_id: Option<CandidateId>,
    /// Internal read outcome: the engine records this exact incident before returning the workspace.
    #[serde(skip)]
    #[ts(skip)]
    pub open_candidate_incident: Option<crate::ContentIncident>,
    /// Captured internal receipt evidence requiring explicit reportability review.
    pub condition_review_source: crate::ConditionReviewSource,
    pub specimen_id: SpecimenId,
    pub report: Option<WorkspaceReport>,
    pub subject: SubjectSummary,
    pub accession_number: String,
    pub specimen_type_code: String,
    pub preview: ReportPreview,
    pub open_candidate: Option<CandidateView>,
    pub allowed_actions: Vec<String>,
}

/// A destination as an obligation shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObligationDestinationView {
    pub destination_id: DestinationId,
    pub channel: DeliveryChannel,
    pub label: String,
    pub verified: bool,
}

/// A status code with its one label; the label names channel and evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeliveryStatusView {
    pub code: ObligationStatus,
    pub label: String,
}

/// One attested attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AttemptView {
    pub attempt_id: DeliveryAttemptId,
    pub evidence: DeliveryEvidence,
    pub observed_at: UtcTimestamp,
    pub recorded_by: ReportUserView,
    pub failure_code: Option<DeliveryFailureCode>,
    // Queue reads redact this; audited report-detail reads retain it.
    pub note: Option<String>,
}

/// One delivery obligation with its status and attempts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObligationView {
    pub obligation_id: ObligationId,
    /// The order's client, whose destinations the obligation may use.
    pub client_id: ClientId,
    pub report_id: ReportId,
    pub report_number: String,
    pub revision_id: ReportRevisionId,
    #[ts(type = "number")]
    pub revision_no: i64,
    pub kind: ObligationKind,
    pub destination: ObligationDestinationView,
    pub status: DeliveryStatusView,
    pub attempts: Vec<AttemptView>,
    pub origin: ObligationOrigin,
    pub origin_reason: Option<String>,
    pub created_at: UtcTimestamp,
    pub allowed_actions: Vec<String>,
}

/// One export of an issued file, labelled as not a delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportView {
    pub export_id: ExportId,
    #[ts(type = "number | null")]
    pub revision_no: Option<i64>,
    pub withdrawal_notice: bool,
    pub purpose: ExportPurpose,
    pub file_name: String,
    pub exported_by: ReportUserView,
    pub exported_at: UtcTimestamp,
    pub label: String,
}

/// One open impact reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImpactReasonView {
    pub key: String,
    pub code: String,
    pub message: String,
}

/// One impact assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImpactAssessmentView {
    pub assessment_id: ImpactAssessmentId,
    pub decision: ImpactDecision,
    pub reason: String,
    pub keys: Vec<String>,
    pub assessed_by: ReportUserView,
    pub assessed_at: UtcTimestamp,
}

/// Impact on the current revision after its release (REQ-RPT-002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImpactView {
    pub revision_id: ReportRevisionId,
    pub open: Vec<ImpactReasonView>,
    /// The latest assessment requires a correction not yet issued.
    pub correction_due: bool,
    pub assessments: Vec<ImpactAssessmentView>,
}

/// A report's withdrawal; the reason is shown here, never on the notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WithdrawalView {
    #[ts(type = "number")]
    pub revision_no: i64,
    pub reason: String,
    pub withdrawn_by: ReportUserView,
    pub withdrawn_at: UtcTimestamp,
    pub notice_artifact_sha256: String,
}

/// A report with its history and delivery.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportDetail {
    pub report_id: ReportId,
    pub report_number: String,
    pub specimen_id: SpecimenId,
    /// The order's client, whose destinations the report may use.
    pub client_id: ClientId,
    pub subject: SubjectSummary,
    pub accession_number: String,
    pub availability: Option<RevisionAvailability>,
    pub revisions: Vec<RevisionSummary>,
    pub obligations: Vec<ObligationView>,
    pub exports: Vec<ExportView>,
    /// `None` when not current, or while its stored content is damaged.
    pub impact: Option<ImpactView>,
    pub withdrawal: Option<WithdrawalView>,
    /// Preservation incidents, open and closed, oldest first.
    pub incidents: Vec<PreservationIncidentView>,
    /// What the caller may do now; the commands still check everything.
    pub allowed_actions: Vec<String>,
}

/// How an issued revision was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RevisionProvenance {
    pub template_id: String,
    #[ts(type = "number")]
    pub template_version: i64,
    pub template_sha256: String,
    pub renderer_version: String,
    pub content_sha256: String,
    pub data_sha256: String,
    pub artifact_sha256: String,
    pub fonts: Vec<String>,
}

/// One revision with its exact printed content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RevisionDetail {
    pub report_id: ReportId,
    pub report_number: String,
    pub summary: RevisionSummary,
    pub content: ToxReportData,
    pub provenance: RevisionProvenance,
}

/// One report in a list: identifiers and states only, no PHI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportSummary {
    pub report_id: ReportId,
    pub report_number: String,
    pub specimen_id: SpecimenId,
    pub accession_number: String,
    #[ts(type = "number")]
    pub latest_revision_no: i64,
    pub availability: RevisionAvailability,
    pub completeness: ReportCompleteness,
    pub released_at: UtcTimestamp,
    /// The query found another row beyond the requested limit. Identical on
    /// every returned row; an empty answer necessarily has no more rows.
    #[ts(optional)]
    pub has_more: Option<bool>,
}

/// A verification of a destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DestinationVerificationView {
    pub by: ReportUserView,
    pub at: UtcTimestamp,
    pub method: String,
    /// Verified by its creator, as the laboratory's separation-of-duties policy
    /// allowed (ADR 0015); by a second person otherwise.
    pub by_creator: bool,
}

/// A delivery destination (REQ-RPT-012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DestinationView {
    pub destination_id: DestinationId,
    pub client_id: ClientId,
    pub client_location_id: Option<ClientLocationId>,
    pub channel: DeliveryChannel,
    pub address: Option<String>,
    pub label: String,
    pub verified: Option<DestinationVerificationView>,
    /// Unverified, verified, reported wrong by an attempt, or retired.
    pub state: DestinationState,
    pub retired: bool,
    pub retire_reason: Option<String>,
    pub replaces_id: Option<DestinationId>,
    pub created_by: ReportUserView,
    #[ts(type = "number")]
    pub revision: i64,
    pub allowed_actions: Vec<String>,
}
