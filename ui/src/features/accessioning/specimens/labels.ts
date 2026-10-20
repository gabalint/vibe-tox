import type { Tone } from "@/components/tones";
import type { CollectionView } from "@/generated/CollectionView";
import type { ContainerView } from "@/generated/ContainerView";
import type { ExcursionOutcome } from "@/generated/ExcursionOutcome";
import type { ExcursionState } from "@/generated/ExcursionState";
import type { LabelJobView } from "@/generated/LabelJobView";
import type { LabelOutcome } from "@/generated/LabelOutcome";
import type { ReceiptDecision } from "@/generated/ReceiptDecision";
import type { ReceiptDiscrepancyCode } from "@/generated/ReceiptDiscrepancyCode";
import type { SpecimenAcceptance } from "@/generated/SpecimenAcceptance";
import type { VolumeKind } from "@/generated/VolumeKind";
import { type Choice, labelOf } from "@/lib/choices";

export const EXCURSION_STATES: readonly Choice<ExcursionState>[] = [
  { id: "open", label: "Open" },
  { id: "assessed", label: "Assessed" },
];

export const EXCURSION_OUTCOMES: readonly Choice<ExcursionOutcome>[] = [
  { id: "no_impact", label: "No impact" },
  { id: "holds_placed", label: "Holds placed" },
];

export const COLLECTION_TIME_KINDS: readonly Choice<"datetime" | "zoned" | "date" | "unknown">[] = [
  { id: "datetime", label: "Date and time (UTC offset supplied)" },
  { id: "zoned", label: "Date and time (named timezone)" },
  { id: "date", label: "Date only; time not recorded" },
  { id: "unknown", label: "Unknown" },
];

export const VOLUME_KINDS: readonly Choice<VolumeKind>[] = [
  { id: "estimated", label: "Estimated" },
  { id: "measured", label: "Measured" },
  { id: "unknown", label: "Unknown" },
];

export const RECEIPT_DECISIONS: readonly Choice<ReceiptDecision>[] = [
  { id: "accepted", label: "Accept" },
  { id: "accepted_conditionally", label: "Accept conditionally" },
  { id: "held", label: "Hold" },
  { id: "rejected", label: "Reject" },
];

export const DISCREPANCIES: readonly Choice<ReceiptDiscrepancyCode>[] = [
  { id: "unlabeled", label: "Unlabeled" },
  { id: "label_mismatch", label: "Label does not match the requisition" },
  { id: "leaking", label: "Leaking" },
  { id: "insufficient_volume", label: "Insufficient volume" },
  { id: "temperature_or_stability", label: "Temperature or stability concern" },
  { id: "container_damaged", label: "Container damaged" },
  { id: "other", label: "Other" },
];

const ACCEPTANCE: Record<SpecimenAcceptance, { label: string; tone: Tone }> = {
  received: { label: "Received, not yet decided", tone: "warning" },
  accepted: { label: "Accepted", tone: "neutral" },
  accepted_conditionally: { label: "Accepted conditionally", tone: "warning" },
  held: { label: "Held", tone: "hold" },
  rejected: { label: "Rejected", tone: "neutral" },
};

export function acceptanceStatus(acceptance: SpecimenAcceptance): { label: string; tone: Tone } {
  return ACCEPTANCE[acceptance];
}

// REQ-SPC-001: `handed_off` is never shown as "printed"; only the printer knows that.
const OUTCOMES: Record<LabelOutcome | "queued", { label: string; tone: Tone }> = {
  queued: { label: "Queued", tone: "info" },
  handed_off: { label: "Handed off to printer", tone: "neutral" },
  failed: { label: "Failed", tone: "danger" },
};

/**
 * Why a label job failed, in the engine's own words for its closed set of PHI-free codes
 * (`lis-engine` work queue labels). A code not in the set shows as sent.
 */
const LABEL_FAILURES: Partial<Record<string, string>> = {
  printer_inactive: "Printer not in service",
  printer_misconfigured: "Printer set up incorrectly",
  printer_unavailable: "Printer unavailable",
  render_failed: "Label could not be drawn",
  invalid_queue: "Print queue not found",
  connection_refused: "Printer refused the connection",
  connect_timeout: "Printer did not answer",
  write_timeout: "Printer stopped answering",
  destination_refused: "Printer refused the label",
  network_error: "Network error",
  spooler_unavailable: "Print service unavailable",
  spooler_rejected: "Print service refused the label",
  spooler_timeout: "Print service did not answer",
};

export function labelJobStatus(job: LabelJobView): { label: string; tone: Tone } {
  const outcome = OUTCOMES[job.outcome ?? "queued"];
  const kind = job.kind === "reprint" ? "Reprint" : "Print";
  const error = job.errorCode ? ` (${LABEL_FAILURES[job.errorCode] ?? job.errorCode})` : "";
  return { tone: outcome.tone, label: `${kind}: ${outcome.label}${error}` };
}

const CONTAINER_STATES: Record<ContainerView["state"], string> = {
  available: "Available",
  depleted: "Depleted",
  disposed: "Disposed",
};

export function containerStateLabel(state: ContainerView["state"]): string {
  return CONTAINER_STATES[state];
}

/** Volume exactly as recorded, with its unit and how it was obtained. */
export function volumeText(container: Pick<ContainerView, "volume" | "volumeUnit" | "volumeKind">) {
  const kind = labelOf(VOLUME_KINDS, container.volumeKind).toLowerCase();
  if (container.volume === null) return `Not recorded (${kind})`;
  const unit = container.volumeUnit ? `\u00A0${container.volumeUnit}` : "\u00A0(no unit recorded)";
  return `${container.volume}${unit}, ${kind}`;
}

/** Collection time as recorded; a missing time or an unknown date is said, never guessed. */
export function collectionTimeText(
  collection: CollectionView,
  formatTime: (timestamp: string) => string,
): string {
  const { precision, collectedAt, utcOffsetMinutes, collectedDate } = collection;
  if (precision === "date" && collectedDate) return `${collectedDate}, time not recorded`;
  if (precision === "datetime" && collectedAt && utcOffsetMinutes !== null) {
    return `${formatTime(collectedAt)} (site clock ${offsetText(utcOffsetMinutes)})`;
  }
  return "Unknown";
}

/** A UTC offset in minutes as `UTC−05:00`. */
function offsetText(minutes: number): string {
  const sign = minutes < 0 ? "−" : "+";
  const absolute = Math.abs(minutes);
  const hours = String(Math.floor(absolute / 60)).padStart(2, "0");
  return `UTC${sign}${hours}:${String(absolute % 60).padStart(2, "0")}`;
}

/** For the decision history: the discrepancy codes in words. */
export function discrepancyText(
  codes: readonly { code: ReceiptDiscrepancyCode; note: string | null }[],
) {
  if (codes.length === 0) return "No discrepancies";
  return codes
    .map(({ code, note }) =>
      note ? `${labelOf(DISCREPANCIES, code)} (${note})` : labelOf(DISCREPANCIES, code),
    )
    .join("; ");
}
