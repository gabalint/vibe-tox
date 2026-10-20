import type { Tone } from "@/components/tones";
import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import type { Choice } from "@/lib/choices";

// Labels for billing engine codes (design brief §5). An unknown code shows as received.

interface Status {
  label: string;
  tone: Tone;
}

function statusOf(statuses: Record<string, Status>, code: string): Status {
  return statuses[code] ?? { label: code, tone: "neutral" };
}

export type WorkFilter = "needs_derivation" | "held" | "ready" | "outstanding" | "rejected";

export const WORK_FILTERS: readonly Choice<WorkFilter>[] = [
  { id: "needs_derivation", label: "Needs derivation" },
  { id: "held", label: "Held" },
  { id: "ready", label: "Ready for approval or handoff" },
  { id: "outstanding", label: "Handed off, awaiting outcome" },
  { id: "rejected", label: "Rejected by the billing partner" },
];

const DERIVATION_STATES: Record<string, Status> = {
  held: { label: "Held", tone: "hold" },
  candidate: { label: "Ready for approval", tone: "warning" },
  approved: { label: "Approved", tone: "neutral" },
  handed_off: { label: "Handed off", tone: "neutral" },
  superseded: { label: "Superseded", tone: "neutral" },
};

const ITEM_STATUSES: Record<string, Status> = {
  outstanding: { label: "Awaiting outcome", tone: "info" },
  acknowledged: { label: "Acknowledged", tone: "neutral" },
  rejected: { label: "Rejected", tone: "warning" },
};

export function derivationState(code: string): Status {
  return statusOf(DERIVATION_STATES, code);
}

export function itemStatus(code: string): Status {
  return statusOf(ITEM_STATUSES, code);
}

/** A billing work entry's state: a derivation state, a handoff item status, or none yet. */
export function workState(code: string): Status {
  if (code === "needs_derivation") return { label: "Needs derivation", tone: "warning" };
  return DERIVATION_STATES[code] ?? itemStatus(code);
}

export const COMPLETENESS: readonly Choice<string>[] = [
  { id: "complete", label: "Complete" },
  { id: "awaiting_final", label: "Awaiting final reports" },
];

export const DOS_RULES: readonly Choice<string>[] = [
  { id: "collection_local_date_v1", label: "Collection date, in the collection's local time" },
];

export const CERTAINTIES: readonly Choice<string>[] = [
  { id: "presumptive", label: "Presumptive (screening)" },
  { id: "definitive", label: "Definitive (confirmation)" },
];

export const DISPOSITIONS: readonly Choice<string>[] = [
  { id: "performed", label: "Performed" },
  { id: "cancelled", label: "Cancelled" },
  { id: "not_performed", label: "Not performed" },
  { id: "failed_only", label: "Failed attempts only" },
  { id: "validity_excluded", label: "Specimen validity (excluded)" },
];

export const FLAGS: readonly Choice<string>[] = [
  { id: "billing_context_missing", label: "Order billing selection missing" },
  { id: "billing_context_incomplete", label: "Order billing selection incomplete" },
  { id: "billing_context_conflict", label: "Same-day order billing contexts need reconciliation" },
  { id: "billing_review_open", label: "Financial follow-up remains open" },
  { id: "collection_review_pending", label: "Collection correction awaits quality review" },
  { id: "mapping_missing", label: "No billing class for an analyte" },
  { id: "date_of_service_unknown", label: "Date of service unknown" },
  { id: "report_not_final", label: "Report not final" },
  { id: "laboratory_billing_identity_missing", label: "Laboratory billing NPI or TIN missing" },
  { id: "ordered_not_performed", label: "Ordered but not performed" },
  {
    id: "presumptive_and_definitive_same_specimen",
    label: "Presumptive and definitive testing on the same specimen",
  },
  { id: "multiple_presumptive_same_dos", label: "More than one presumptive test that day" },
  { id: "performed_not_ordered", label: "Performed but not ordered (not counted)" },
  { id: "failed_attempt_only", label: "Only failed attempts (not counted)" },
  { id: "performed_not_reported", label: "Performed but not reported" },
];

export const ITEM_KINDS: readonly Choice<string>[] = [
  { id: "original", label: "Original" },
  { id: "adjustment", label: "Adjustment" },
];

export type HandoffOutcomeCode = "acknowledged" | "rejected";

export const OUTCOMES: readonly Choice<HandoffOutcomeCode>[] = [
  { id: "acknowledged", label: "Acknowledged by the billing partner" },
  { id: "rejected", label: "Rejected by the billing partner" },
];

export function yesNo(value: boolean): string {
  return value ? "Yes" : "No";
}

/** A mapping version's status, whether it is the one in effect, and what it awaits. */
export interface MappingStatus extends Status {
  /** The version in effect: the current mapping. */
  current: boolean;
  /** What the version waits for before it can take effect, in words; `null` when nothing. */
  awaiting: string | null;
}

/**
 * A mapping version's status in words, from the status and dates the engine returned: a draft
 * awaits approval, an approved version awaits the laboratory's confirmation until confirmed,
 * is current while its effective period is open, and replaced once another confirmation
 * ended it.
 */
export function mappingStatus(
  version: Pick<
    BillingClassMappingVersionSummary,
    "status" | "confirmedAt" | "effectiveFrom" | "effectiveTo"
  >,
): MappingStatus {
  const of = (label: string, tone: Tone, current = false, awaiting: string | null = null) => ({
    label,
    tone,
    current,
    awaiting,
  });
  if (version.status === "draft") return of("Draft", "warning", false, "awaiting approval");
  if (version.status === "retired") return of("Retired", "neutral");
  if (version.status !== "approved") return of(version.status, "neutral");
  if (version.confirmedAt === null) {
    return of("Awaiting confirmation", "warning", false, "awaiting confirmation");
  }
  if (version.effectiveFrom !== null && version.effectiveTo === null) {
    return of("Current", "neutral", true);
  }
  return of("Replaced", "neutral");
}
