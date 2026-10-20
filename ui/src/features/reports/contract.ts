// Report preparation, release, history and delivery (REQ-RPT-001, REQ-RPT-002, REQ-RPT-003,
// REQ-RPT-005, REQ-RPT-011, REQ-RPT-012, REQ-RPT-013; design brief §3, §4 and §7). The engine
// builds every printed string (`ToxReportData`, generated from `lis-report`), every gate and
// every delivery label; screens show them as received. Preparing and releasing need
// `release_reports`; delivery needs `deliver_reports`; report history needs `deliver_reports` or
// `manage_quality` (impact assessment). Workspace, candidate, report and revision reads are
// audited accesses.
import type { CommandName } from "@/generated/CommandName";
import type { ObligationView } from "@/generated/ObligationView";

export const RELEASE_REPORTS = "release_reports";

/** `get_report_workspace({input:{specimenId}}) → ReportWorkspace` (audited read). */
export const GET_REPORT_WORKSPACE = "get_report_workspace";
/** `prepare_report_candidate({input:{specimenId, amendmentReason?, conditionReview?}}) → CandidateView`. */
export const PREPARE_REPORT_CANDIDATE = "prepare_report_candidate";
/** `get_report_candidate({input:{candidateId}}) → CandidateView` (audited read). */
export const GET_REPORT_CANDIDATE = "get_report_candidate";
/** `release_report({input: ReleaseReportInput}) → RevisionSummary`. */
export const RELEASE_REPORT = "release_report";
/** `withdraw_report({input: WithdrawReportInput}) → ReportDetail`. */
export const WITHDRAW_REPORT = "withdraw_report";
/**
 * `list_reports({input:{filter, search?, limit, beforeReleasedAt?, beforeReportId?}}) → ReportSummary[]`, latest
 * release first; `hasMore` on every row says whether an older page exists.
 */
export const LIST_REPORTS = "list_reports";
/** `get_report({input:{reportId}}) → ReportDetail` (audited read). */
export const GET_REPORT = "get_report";
/** `get_report_revision({input:{revisionId}}) → RevisionDetail` (audited read). */
export const GET_REPORT_REVISION = "get_report_revision";
/** `export_report_pdf({input:{revisionId, purpose, obligationId?}}) → ExportedReportFile`. */
export const EXPORT_REPORT_PDF = "export_report_pdf";
/** `record_impact_assessment({input:{revisionId, keys, decision, reason}})`. */
export const RECORD_IMPACT_ASSESSMENT = "record_impact_assessment";
/**
 * `close_preservation_incident({input:{incidentId, expectedRevision?, resolution, reason}}) →
 * PreservationIncidentView` (`manage_quality`).
 */
export const CLOSE_PRESERVATION_INCIDENT = "close_preservation_incident";
/** `list_delivery_obligations({input:{filter, limit, after?}}) → DeliveryQueuePage`. */
export const LIST_DELIVERY_OBLIGATIONS = "list_delivery_obligations";
/** Audited exact retained obligation, independent of queue filters/pages. */
export const GET_DELIVERY_OBLIGATION = "get_delivery_obligation";
/** `record_delivery_attempt({input: RecordDeliveryAttemptInput}) → ObligationView`. */
export const RECORD_DELIVERY_ATTEMPT = "record_delivery_attempt";
/** `add_delivery_obligation({input:{revisionId, destinationId, reason}}) → ObligationView`. */
export const ADD_DELIVERY_OBLIGATION = "add_delivery_obligation";
/** `redirect_delivery_obligation({input:{obligationId, destinationId, reason}}) → ObligationView`. */
export const REDIRECT_DELIVERY_OBLIGATION = "redirect_delivery_obligation";
/** `list_delivery_destinations({input:{clientId?, state?}}) → DestinationView[]` (session). */
export const LIST_DELIVERY_DESTINATIONS = "list_delivery_destinations";
/** `list_work_queues() → WorkQueueSummary[]`, filtered by the caller's capabilities. */
export const LIST_WORK_QUEUES = "list_work_queues";
/** `list_work_queue_items({input:{queue, limit}}) → WorkQueuePage`, oldest first. */
const LIST_WORK_QUEUE_ITEMS = "list_work_queue_items";

export const LIST_LIMIT = 200;

// Views name the actions the engine allows the signed-in person by their command names, such
// as `allowedActions: ["release_report"]`. Screens offer an action only when it is named.

/** What a candidate change makes stale. */
export const CANDIDATE_QUERIES: readonly CommandName[] = [
  GET_REPORT_WORKSPACE,
  GET_REPORT_CANDIDATE,
  LIST_WORK_QUEUES,
  LIST_WORK_QUEUE_ITEMS,
];
/** What a release, withdrawal or impact decision makes stale. */
export const REPORT_QUERIES: readonly CommandName[] = [
  ...CANDIDATE_QUERIES,
  LIST_REPORTS,
  GET_REPORT,
  GET_REPORT_REVISION,
  LIST_DELIVERY_OBLIGATIONS,
  GET_DELIVERY_OBLIGATION,
];
/** What a delivery attempt, new obligation or redirect makes stale. */
export const DELIVERY_QUERIES: readonly CommandName[] = [
  GET_REPORT,
  LIST_DELIVERY_OBLIGATIONS,
  GET_DELIVERY_OBLIGATION,
  LIST_WORK_QUEUES,
  LIST_WORK_QUEUE_ITEMS,
];

export type { CandidateState } from "@/generated/CandidateState";
export type { CandidateView } from "@/generated/CandidateView";
export type { ClosePreservationIncidentInput } from "@/generated/ClosePreservationIncidentInput";
export type { DeliveryEvidence } from "@/generated/DeliveryEvidence";
export type { DeliveryFailureCode } from "@/generated/DeliveryFailureCode";
export type { DeliveryQueueFilter } from "@/generated/DeliveryQueueFilter";
export type { DeliveryQueuePage } from "@/generated/DeliveryQueuePage";
export type { DestinationView } from "@/generated/DestinationView";
export type { DispositionView } from "@/generated/DispositionView";
export type { ExportedReportFile } from "@/generated/ExportedReportFile";
export type { ExportPurpose } from "@/generated/ExportPurpose";
export type { ExportReportPdfInput } from "@/generated/ExportReportPdfInput";
export type { ImpactDecision } from "@/generated/ImpactDecision";
export type { ImpactView } from "@/generated/ImpactView";
export type { IncidentResolution } from "@/generated/IncidentResolution";
export type { LineDisposition } from "@/generated/LineDisposition";
export type { ObligationOrigin } from "@/generated/ObligationOrigin";
export type { PreservationIncidentKind } from "@/generated/PreservationIncidentKind";
export type { PreservationIncidentView } from "@/generated/PreservationIncidentView";
export type { RecordDeliveryAttemptInput } from "@/generated/RecordDeliveryAttemptInput";
export type { RecordImpactAssessmentInput } from "@/generated/RecordImpactAssessmentInput";
export type { ReleaseReportInput } from "@/generated/ReleaseReportInput";
export type { ReportCompleteness } from "@/generated/ReportCompleteness";
export type { ReportDetail } from "@/generated/ReportDetail";
export type { ReportListFilter } from "@/generated/ReportListFilter";
export type { ReportRelationship } from "@/generated/ReportRelationship";
export type { ReportSummary } from "@/generated/ReportSummary";
export type { ReportWorkspace } from "@/generated/ReportWorkspace";
export type { RevisionAvailability } from "@/generated/RevisionAvailability";
export type { RevisionDetail } from "@/generated/RevisionDetail";
export type { RevisionSummary } from "@/generated/RevisionSummary";
export type { StoredRecord } from "@/generated/StoredRecord";
export type { WithdrawReportInput } from "@/generated/WithdrawReportInput";
export type { WorkQueueItem } from "@/generated/WorkQueueItem";
export type { WorkQueueSummary } from "@/generated/WorkQueueSummary";
export type { WorkspaceReport } from "@/generated/WorkspaceReport";
export type { ObligationView };
