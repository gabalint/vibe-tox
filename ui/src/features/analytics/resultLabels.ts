import type { ImportFailureCode } from "@/generated/ImportFailureCode";
import type { ImportOutcome } from "@/generated/ImportOutcome";
import type { MeasurementView } from "@/generated/MeasurementView";
import type { MethodCertainty } from "@/generated/MethodCertainty";
import type { OutputScale } from "@/generated/OutputScale";
import type { QuarantineReason } from "@/generated/QuarantineReason";
import type { QuarantineState } from "@/generated/QuarantineState";
import type { ReflexDecision } from "@/generated/ReflexDecision";
import type { ResultCause } from "@/generated/ResultCause";
import type { ResultOutcome } from "@/generated/ResultOutcome";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import type { ReviewQueueFilter } from "@/generated/ReviewQueueFilter";
import type { ReviewStatus } from "@/generated/ReviewStatus";
import type { SequenceRole } from "@/generated/SequenceRole";
import type { SpecimenAcceptance } from "@/generated/SpecimenAcceptance";
import { type Choice, labelOf } from "@/lib/choices";
import { ATTEMPT_KINDS, receivedValueText, type Status, valueWithUnits } from "./labels";

const OUTCOME_WORDS: Record<ResultOutcome, string> = {
  quantity: "Quantity",
  censored: "Censored",
  qualitative: "Qualitative",
  not_performed: "Not performed",
  invalid: "Invalid",
  indeterminate: "Indeterminate",
  missing: "Missing",
};

/**
 * A revision's value exactly as the engine sent it, with its units (REQ-RES-001): the
 * magnitude is decimal text and is never reformatted, and a comparator is shown as received.
 */
export function resultValueText(revision: ResultRevisionView): string {
  switch (revision.outcome) {
    case "quantity":
      return valueWithUnits(revision.magnitude ?? "", revision.units);
    case "censored":
      return valueWithUnits(
        `${revision.comparator ?? ""}${revision.magnitude ?? ""}`,
        revision.units,
      );
    case "qualitative":
      return revision.code ?? "";
    default:
      return OUTCOME_WORDS[revision.outcome];
  }
}

const NOT_CLASSIFIABLE_REASONS: Record<string, string> = {
  rounding_rule_not_configured: "no approved rounding rule for this cutoff",
  censored_bound_inconclusive: "the reported bound does not decide the comparison",
};

/**
 * The cutoff classification in plain words. It compares the value with a configured number
 * and is not a clinical interpretation (REQ-RES-003). Unknown codes show as received.
 */
export function classificationText(revision: ResultRevisionView): string {
  switch (revision.classification) {
    case "meets_cutoff":
      return "Meets cutoff";
    case "does_not_meet_cutoff":
      return "Does not meet cutoff";
    case "not_applicable":
      return "No cutoff applies";
    case "not_classifiable": {
      const reason = revision.classificationReason ?? "reason not given";
      return `Not classifiable: ${NOT_CLASSIFIABLE_REASONS[reason] ?? reason}`;
    }
    default:
      return revision.classification;
  }
}

const CAUSES: Record<ResultCause, string> = {
  initial: "From the first import",
  reprocessed: "From a reprocessed import",
  selected: "Evidence selected by a reviewer",
  disposition: "Disposition recorded by a reviewer",
};

export function causeText(cause: ResultCause): string {
  return CAUSES[cause];
}

export function methodText(revision: ResultRevisionView): string {
  const certainty: Record<MethodCertainty, string> = {
    presumptive: "Presumptive",
    definitive: "Definitive",
  };
  const scale: Record<OutputScale, string> = {
    qualitative: "qualitative",
    quantitative: "quantitative",
  };
  return `${certainty[revision.methodCertainty]}, ${scale[revision.outputScale]}, ${labelOf(SEQUENCE_ROLES, revision.sequenceRole).toLowerCase()}`;
}

export const SEQUENCE_ROLES: readonly Choice<SequenceRole>[] = [
  { id: "first_line", label: "First-line test" },
  { id: "confirmation", label: "Confirmation" },
];

/**
 * A person's decision on a reflex trigger (REQ-ORD-003). The cutoff classification is shown
 * beside it but never decides it.
 */
export const REFLEX_DECISIONS: readonly Choice<ReflexDecision>[] = [
  { id: "condition_met", label: "Condition met: request the reflex test" },
  { id: "condition_not_met", label: "Condition not met: no reflex test" },
];

/** Outcomes a reviewer may record instead of a source value (REQ-RES-001). */
export const DISPOSITIONS: readonly Choice<ResultOutcome>[] = [
  { id: "invalid", label: "Invalid" },
  { id: "indeterminate", label: "Indeterminate" },
  { id: "missing", label: "Missing" },
  { id: "not_performed", label: "Not performed" },
];

/** One measurement as the source reported it: value, units and status are plain text. */
export function measurementText(measurement: MeasurementView): string {
  const status = measurement.statusText ? `, status ${measurement.statusText}` : "";
  return `${receivedValueText(measurement)}${status}`;
}

export function measurementSource(measurement: MeasurementView): string {
  const attempt = labelOf(ATTEMPT_KINDS, measurement.attemptKind).toLowerCase();
  return `Position ${measurement.positionLabel}, ${attempt} attempt, processing ${measurement.processingRevisionSeq}`;
}

export function reviewStatus(status: ReviewStatus): Status {
  if (status.status === "current") return { label: "Reviewed", tone: "neutral" };
  if (status.status === "stale") return { label: "Review out of date", tone: "warning" };
  return { label: "Not reviewed", tone: "neutral" };
}

export const QUEUE_FILTERS: readonly Choice<ReviewQueueFilter>[] = [
  { id: "ready", label: "Ready for review" },
  { id: "stale", label: "Review out of date" },
  { id: "blocked", label: "Blocked" },
  { id: "quarantine", label: "Quarantined rows" },
];

const QUARANTINE_REASONS: Record<QuarantineReason, string> = {
  unknown_sample: "No sample on a run has this label",
  ambiguous_sample: "More than one sample matches this label",
  run_closed: "The run was closed before this row arrived",
  unknown_analyte: "The test code is not mapped to an analyte",
  not_in_ordered_scope: "The analyte was not ordered for this sample",
  incompatible_unit: "The units differ from the method's units",
  malformed_value: "The value could not be read",
  scale_mismatch: "The value form does not match the method's output scale",
  conflicting_duplicate: "A different value arrived for the same row",
  dilution_not_supported: "The source does not report final concentration for dilutions",
  execution_failed: "The analysis attempt for this sample was marked failed",
  // Result files (brief §5, F9, F11).
  malformed_row: "The row has a different number of fields than the file's header",
  role_mismatch: "The sample type in the file differs from the run position's role",
  dilution_mismatch: "The dilution factor in the file differs from the one recorded for the test",
};

export function quarantineReasonText(reason: QuarantineReason): string {
  return QUARANTINE_REASONS[reason];
}

const QUARANTINE_STATES: Record<QuarantineState, Status> = {
  open: { label: "Needs a decision", tone: "warning" },
  accepted_as_reprocessing: { label: "Accepted as reprocessing", tone: "neutral" },
  rejected: { label: "Rejected", tone: "neutral" },
  superseded_by_retry: { label: "Replaced by a retry", tone: "neutral" },
};

export function quarantineStatus(state: QuarantineState): Status {
  return QUARANTINE_STATES[state];
}

const IMPORT_OUTCOMES: Record<ImportOutcome, Status> = {
  processed: { label: "Processed", tone: "neutral" },
  processed_with_quarantine: { label: "Processed, some rows quarantined", tone: "warning" },
  quarantined: { label: "Quarantined", tone: "danger" },
};

export function importStatus(outcome: ImportOutcome): Status {
  return IMPORT_OUTCOMES[outcome];
}

/** Why nothing was imported, and what to do next (result files brief §5, F12). */
const FAILURES: Record<ImportFailureCode, string> = {
  malformed_message:
    "The message could not be read. Ask for it to be sent again from the analyzer.",
  unregistered_source:
    "The instrument has no approved source: no approved sender, or no approved file template. Approve one, then use Retry import.",
  sender_mismatch:
    "The sender name or version differs from the approved source. Approve a source for this sender and version, then use Retry import.",
  undecodable_text:
    "The file is not text in the template's encoding. Save the export as UTF-8, or approve a template version that reads Windows-1252, then use Retry import.",
  layout_mismatch:
    "The file's columns differ from the approved template. Fix the export or approve a new template version, then use Retry import.",
  limits_exceeded:
    "The file has more rows, columns or characters in one value than the LIS reads: 50,000 rows, 512 columns, 4 KiB per value. Export the batch in smaller files and load each one.",
};

export function failureText(code: ImportFailureCode | null): string {
  return code === null ? "None" : FAILURES[code];
}

/** How receipt decided on the specimen, in words; review shows it beside the results. */
export const SPECIMEN_ACCEPTANCE: readonly Choice<SpecimenAcceptance>[] = [
  { id: "received", label: "Received, not yet decided" },
  { id: "accepted", label: "Accepted" },
  { id: "accepted_conditionally", label: "Accepted conditionally" },
  { id: "held", label: "Held" },
  { id: "rejected", label: "Rejected" },
];
