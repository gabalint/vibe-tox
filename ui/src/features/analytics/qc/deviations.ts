import type { DeviationView } from "@/generated/DeviationView";
import { labelOf } from "@/lib/choices";
import { DEVIATION_SCOPES, type Status } from "../labels";
import type { QcTargets } from "./OpenDeviationDialog";

export function deviationStatus(deviation: DeviationView): Status {
  return deviation.state === "open"
    ? { label: "Open", tone: "warning" }
    : { label: "Closed", tone: "neutral" };
}

/** What the deviation applies to, named as the run's QC names it. */
export function scopeText(deviation: DeviationView, targets: QcTargets): string {
  if (deviation.scope === "run") return "This run";
  const target = [...targets.positions, ...targets.lots].find(
    (candidate) => candidate.id === deviation.scopeId,
  );
  return target?.label ?? `${labelOf(DEVIATION_SCOPES, deviation.scope)} ${deviation.scopeId}`;
}

/** The one analyte the deviation is limited to, if any. */
export function analyteText(deviation: DeviationView, targets: QcTargets): string {
  if (deviation.analyteId === null) return "Not limited to one analyte";
  const analyte = targets.analytes.find((candidate) => candidate.id === deviation.analyteId);
  return analyte?.label ?? deviation.analyteId;
}
