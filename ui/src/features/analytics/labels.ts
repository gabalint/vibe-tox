import type { Tone } from "@/components/tones";
import type { AttemptKind } from "@/generated/AttemptKind";
import type { DeviationScope } from "@/generated/DeviationScope";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { InstrumentStatus } from "@/generated/InstrumentStatus";
import type { LotView } from "@/generated/LotView";
import type { MaterialKind } from "@/generated/MaterialKind";
import type { PositionRole } from "@/generated/PositionRole";
import type { QcOutcome } from "@/generated/QcOutcome";
import type { QcRole } from "@/generated/QcRole";
import type { RegistrationState } from "@/generated/RegistrationState";
import type { RevisionIndicator } from "@/generated/RevisionIndicator";
import type { RunDetail } from "@/generated/RunDetail";
import type { RunState } from "@/generated/RunState";
import type { SampleIdField } from "@/generated/SampleIdField";
import type { Choice } from "@/lib/choices";

/** A state in words with its tone; the badge never relies on color alone. */
export interface Status {
  label: string;
  tone: Tone;
}

export const INSTRUMENT_STATUSES: readonly Choice<InstrumentStatus>[] = [
  { id: "in_service", label: "In service" },
  { id: "out_of_service", label: "Out of service" },
];

/** The instrument's approved result source, as its sender name and version. */
export function sourceText(instrument: InstrumentReadinessView): string {
  const source = instrument.activeRegistration;
  return source === null ? "None approved" : `${source.sender} ${source.version}`;
}

export const MATERIAL_KINDS: readonly Choice<MaterialKind>[] = [
  { id: "reagent", label: "Reagent" },
  { id: "calibrator", label: "Calibrator" },
  { id: "control", label: "Control" },
  { id: "internal_standard", label: "Internal standard" },
];

/** A lot as the bench names it: "Synthetic Urine Control, lot CTL-2610-L1". */
export function lotText(lot: Pick<LotView, "productName" | "lotNumber">): string {
  return `${lot.productName}, lot ${lot.lotNumber}`;
}

export function lotStatus(lot: LotView): Status {
  if (lot.state === "withdrawn") return { label: "Withdrawn", tone: "neutral" };
  if (lot.expired) return { label: "Expired", tone: "neutral" };
  return { label: "Active", tone: "neutral" };
}

export const QC_ROLES: readonly Choice<QcRole>[] = [
  { id: "control", label: "Control" },
  { id: "calibrator", label: "Calibrator" },
  { id: "blank", label: "Blank" },
];

export const POSITION_ROLES: readonly Choice<PositionRole>[] = [
  { id: "patient", label: "Patient" },
  ...QC_ROLES,
];

export const ATTEMPT_KINDS: readonly Choice<AttemptKind>[] = [
  { id: "initial", label: "Initial" },
  { id: "rerun", label: "Rerun" },
  { id: "reextraction", label: "Re-extraction" },
  { id: "dilution", label: "Dilution" },
];

/** A run's title: its method version and instrument, or "Run" until they load. */
export function runTitle(run: RunDetail | undefined): string {
  return run
    ? `Run of ${run.methodCode} version ${run.methodVersion} on ${run.instrumentName}`
    : "Run";
}

export function runStatus(state: RunState): Status {
  return state === "open"
    ? { label: "Open", tone: "neutral" }
    : { label: "Closed", tone: "neutral" };
}

const REGISTRATION_STATUSES: Record<RegistrationState, Status> = {
  draft: { label: "Draft, awaiting approval", tone: "warning" },
  active: { label: "Active", tone: "neutral" },
  retired: { label: "Retired", tone: "neutral" },
};

export function registrationStatus(state: RegistrationState): Status {
  return REGISTRATION_STATUSES[state];
}

export const SAMPLE_ID_FIELDS: readonly Choice<SampleIdField>[] = [
  { id: "specimen_id", label: "Specimen ID field" },
  { id: "instrument_specimen_id", label: "Instrument specimen ID field" },
];

export const REVISION_INDICATORS: readonly Choice<RevisionIndicator>[] = [
  { id: "none", label: "The analyzer does not mark corrections" },
  { id: "result_status_correction", label: "Corrections carry a corrected result status" },
];

/** Said in place of units the source or the revision did not report. */
const NO_UNITS = "(no units reported)";

/**
 * A value exactly as the engine sent it and its units, joined by a non-breaking space so
 * they stay on one line (UI standards, clinical display rules). Neither is reformatted.
 */
export function valueWithUnits(value: string, units: string | null): string {
  return `${value}\u00A0${units ?? NO_UNITS}`;
}

/** An analyzer value and its units, exactly as received. */
export function receivedValueText(row: { valueText: string | null; unitsText: string | null }) {
  return valueWithUnits(row.valueText ?? "(no value)", row.unitsText);
}

export const QC_OUTCOMES: readonly Choice<QcOutcome>[] = [
  { id: "accepted", label: "Accept" },
  { id: "rejected", label: "Reject" },
];

export function qcOutcomeStatus(outcome: QcOutcome): Status {
  return outcome === "accepted"
    ? { label: "Accepted", tone: "neutral" }
    : { label: "Rejected", tone: "danger" };
}

export const DEVIATION_SCOPES: readonly Choice<DeviationScope>[] = [
  { id: "instrument", label: "Instrument" },
  { id: "lot", label: "Lot" },
  { id: "prep_batch", label: "Preparation batch" },
  { id: "run", label: "Run" },
  { id: "position", label: "Position" },
];
