import type { QcConflictState } from "@/generated/QcConflictState";
import type { QcConflictView } from "@/generated/QcConflictView";
import type { QcObservationView } from "@/generated/QcObservationView";
import type { RunQcView } from "@/generated/RunQcView";
import { receivedValueText, type Status } from "../labels";

/** A changed QC row with its analyte's code and the stored observation it contradicts. */
export interface ConflictRow extends QcConflictView {
  analyteCode: string;
  /** The stored observation as received, at its position. */
  stored: string;
}

export const CONFLICT_STATES: Record<QcConflictState, Status> = {
  open: { label: "Awaits a decision", tone: "warning" },
  accepted: { label: "Accepted as correction", tone: "neutral" },
  rejected: { label: "Rejected", tone: "neutral" },
};

/** The run's changed QC rows in the engine's order, open ones first. */
export function conflictRows(view: RunQcView): ConflictRow[] {
  const observations = new Map<string, QcObservationView>(
    [...view.analytes.flatMap((analyte) => analyte.observations), ...view.unmappedObservations].map(
      (observation) => [observation.observationId, observation],
    ),
  );
  return view.conflicts.map((conflict) => {
    const observation = observations.get(conflict.conflictsWithId);
    return {
      ...conflict,
      analyteCode:
        view.analytes.find((analyte) => analyte.analyteId === conflict.analyteId)?.code ??
        "Not mapped",
      stored: observation
        ? `${receivedValueText(observation)} at ${observation.positionLabel}`
        : conflict.conflictsWithId,
    };
  });
}

/** How people tell changed rows apart: the analyte and the changed value as received. */
export const conflictName = (row: ConflictRow) => `${row.analyteCode}, ${receivedValueText(row)}`;
