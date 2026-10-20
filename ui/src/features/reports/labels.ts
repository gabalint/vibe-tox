import type { Tone } from "@/components/tones";
import type { ToxOtherTestStatus } from "@/generated/ToxOtherTestStatus";
import type { Choice } from "@/lib/choices";
import type {
  CandidateState,
  DeliveryEvidence,
  DeliveryFailureCode,
  DeliveryQueueFilter,
  ExportPurpose,
  ImpactDecision,
  IncidentResolution,
  LineDisposition,
  ObligationOrigin,
  ObligationView,
  PreservationIncidentKind,
  ReportCompleteness,
  ReportListFilter,
  ReportRelationship,
  RevisionAvailability,
  StoredRecord,
} from "./contract";

// Engine codes in laboratory terms: the one place the reports feature labels them. Delivery
// status labels are never built here; they come from the engine (REQ-RPT-011).

export const AVAILABILITY: Record<RevisionAvailability, { label: string; tone: Tone }> = {
  current: { label: "Current", tone: "neutral" },
  superseded: { label: "Superseded", tone: "neutral" },
  withdrawn: { label: "Withdrawn", tone: "neutral" },
};

export const COMPLETENESS: Record<ReportCompleteness, { label: string; tone: Tone }> = {
  final: { label: "Final", tone: "neutral" },
  partial: { label: "Partial", tone: "warning" },
};

export const RELATIONSHIP: Record<ReportRelationship, string> = {
  initial: "Initial report",
  supplement: "Supplement",
  correction: "Correction",
};

export const CANDIDATE_STATE: Record<CandidateState, { label: string; tone: Tone }> = {
  rendering: { label: "Rendering", tone: "info" },
  // "ready" means rendered, not releasable: gates are shown beside it (UI review finding 4).
  ready: { label: "Prepared", tone: "neutral" },
  released: { label: "Released", tone: "neutral" },
  superseded: { label: "Superseded", tone: "neutral" },
  failed: { label: "Rendering failed", tone: "danger" },
};

export const DISPOSITION: Record<LineDisposition, { label: string; tone: Tone }> = {
  reported: { label: "Reported", tone: "neutral" },
  pending: { label: "Pending", tone: "info" },
  held: { label: "Held", tone: "hold" },
  cancelled: { label: "Cancelled", tone: "neutral" },
  not_activated: { label: "Not activated", tone: "neutral" },
  trigger_pending: { label: "Reflex decision pending", tone: "info" },
};

export const OTHER_TEST_STATUS: Record<ToxOtherTestStatus, string> = {
  pending: "Pending",
  held: "Held",
  cancelled: "Cancelled",
  not_activated: "Not activated",
  trigger_pending: "Reflex decision pending",
};

export const EVIDENCE: readonly Choice<DeliveryEvidence>[] = [
  { id: "handed_off", label: "Handed off (I attest it was sent or handed over)" },
  { id: "failed", label: "Failed" },
  { id: "outcome_unknown", label: "Outcome unknown" },
];

export const FAILURE_CODES: readonly Choice<DeliveryFailureCode>[] = [
  { id: "no_answer_or_busy", label: "No answer or busy" },
  { id: "wrong_destination", label: "Wrong destination" },
  { id: "returned", label: "Returned" },
  { id: "refused", label: "Refused" },
  { id: "other", label: "Other" },
];

export const EXPORT_PURPOSES: readonly Choice<ExportPurpose>[] = [
  { id: "delivery", label: "For delivery to this destination" },
  { id: "records", label: "For the laboratory's records" },
  { id: "internal", label: "Internal use" },
];

export const ORIGIN: Record<ObligationOrigin, string> = {
  release: "Release",
  correction_follow_up: "Follow-up of an earlier revision",
  withdrawal: "Withdrawal notice",
  added: "Added by staff",
  redirected: "Redirected",
};

const OBLIGATION_KIND: Record<ObligationView["kind"], string> = {
  report: "Report",
  withdrawal_notice: "Withdrawal notice",
};

/** What an obligation sends: the report or its withdrawal notice, at one revision. */
export function sendsText(obligation: Pick<ObligationView, "kind" | "revisionNo">): string {
  return `${OBLIGATION_KIND[obligation.kind]}, revision ${obligation.revisionNo}`;
}

/** What a recorded attempt says happened; the person who recorded it attests it. */
export const ATTEMPT_EVIDENCE: Record<DeliveryEvidence, string> = {
  handed_off: "Handed off",
  failed: "Failed",
  outcome_unknown: "Outcome unknown",
};

export const DELIVERY_FILTERS: readonly Choice<DeliveryQueueFilter>[] = [
  { id: "awaiting", label: "Awaiting delivery" },
  { id: "attention", label: "Failed or outcome unknown" },
  { id: "blocked", label: "Blocked by destination" },
  { id: "paused", label: "Paused for impact review" },
  { id: "all", label: "All open" },
];

export const REPORT_FILTERS: readonly Choice<ReportListFilter>[] = [
  { id: "all", label: "All reports" },
  { id: "partial", label: "Partial reports" },
  { id: "withdrawn", label: "Withdrawn reports" },
];

export const IMPACT_DECISIONS: readonly Choice<ImpactDecision>[] = [
  { id: "no_impact", label: "No impact on the released report" },
  { id: "correction_required", label: "A correction is required" },
];

export const INCIDENT_KIND: Record<PreservationIncidentKind, string> = {
  missing: "Missing",
  corrupt: "Damaged",
};

export const STORED_RECORD: Record<StoredRecord, string> = {
  pdf: "PDF file",
  content_json: "stored report content",
  data_json: "stored printed data",
};

export const INCIDENT_RESOLUTIONS: readonly Choice<IncidentResolution>[] = [
  { id: "artifact_restored_from_backup", label: "Restored intact from a backup" },
  { id: "revision_reissued", label: "A new revision was issued in its place" },
  { id: "accepted_as_lost", label: "Accepted as lost" },
];

export function revisionText(revisionNo: number, relationship: ReportRelationship): string {
  return `Revision ${revisionNo} (${RELATIONSHIP[relationship].toLowerCase()})`;
}
