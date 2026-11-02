import type { QcAssessmentHistory } from "@/generated/QcAssessmentHistory";
import type { QcObservationView } from "@/generated/QcObservationView";
import type { RunQcView } from "@/generated/RunQcView";
import { qcOutcomeStatus, type Status } from "../labels";
import type { QcTarget } from "./AssessQcDialog";

/** One thing QC is assessed for: an analyte on the run, the whole run, or a prep batch. */
export interface QcScope {
  /** Stable within the run, and kept in the address when the scope is open. */
  key: string;
  /** Short name for the grid, such as the analyte code. */
  label: string;
  /** Heading of the scope's detail and title of its assessment dialog. */
  title: string;
  target: QcTarget;
  /** For the whole run: the observations whose test code maps to no analyte. */
  observations: readonly QcObservationView[];
  assessments: QcAssessmentHistory;
}

/** The whole run's scope key, also its `?open=` value. */
export const RUN_SCOPE_KEY = "run";

/** Every assessable scope of the run, in the engine's order: analytes, the run, batches. */
export function qcScopes(view: RunQcView): QcScope[] {
  const { runId } = view;
  return [
    ...view.analytes.map((analyte) => ({
      key: analyte.analyteId,
      label: analyte.code,
      title: `QC for ${analyte.code}`,
      target: { scope: "run_analyte" as const, runId, analyteId: analyte.analyteId },
      observations: analyte.observations,
      assessments: analyte.assessments,
    })),
    {
      key: RUN_SCOPE_KEY,
      label: "Whole run",
      title: "QC for the whole run",
      target: { scope: "run", runId },
      observations: view.unmappedObservations,
      assessments: view.runAssessments,
    },
    ...view.prepBatches.map((batch) => ({
      key: batch.prepBatchId,
      label: `Preparation batch ${batch.prepBatchId}`,
      title: `Preparation batch ${batch.prepBatchId}`,
      target: { scope: "prep_batch" as const, prepBatchId: batch.prepBatchId },
      observations: [],
      assessments: batch.assessments,
    })),
  ];
}

/** REQ-ANA-004: a run × analyte assessment cites exactly the current observations. */
export function currentObservations(scope: QcScope): QcObservationView[] {
  return scope.observations.filter((observation) => !observation.superseded);
}

/** The scope's current assessment, or that it has none yet; the whole run's is optional. */
export function scopeStatus(scope: QcScope): Status {
  const { current } = scope.assessments;
  if (current !== null) return qcOutcomeStatus(current.outcome);
  return {
    label: scope.key === RUN_SCOPE_KEY ? "Not assessed (optional)" : "Not assessed",
    tone: "neutral",
  };
}

/** The current accepted assessment cites exactly the current observations. */
function acceptanceCurrent(scope: QcScope): boolean {
  const { current } = scope.assessments;
  if (current?.outcome !== "accepted") return false;
  const cited = new Set(current.evidenceObservationIds);
  const shown = currentObservations(scope);
  return cited.size === shown.length && shown.every((each) => cited.has(each.observationId));
}

/**
 * Why the scope cannot be accepted together with other analytes, or null when it can
 * (REQ-ANA-004). Only an analyte qualifies, with observations, no changed QC row or open
 * deviation, and no current acceptance or rejection; the engine still decides each one.
 */
export function bulkAcceptBlocker(scope: QcScope, view: RunQcView): string | null {
  const { target } = scope;
  if (target.scope !== "run_analyte") return "Assessed on its own.";
  const { analyteId } = target;
  if (scope.assessments.current?.outcome === "rejected") return "Rejected; this is final.";
  if (acceptanceCurrent(scope)) return "Already accepted.";
  if (currentObservations(scope).length === 0) return "No QC observations.";
  if (view.conflicts.some((row) => row.state === "open" && row.analyteId === analyteId)) {
    return "A changed QC row awaits a decision.";
  }
  const deviationOpen = view.deviations.some(
    (each) => each.state === "open" && (each.analyteId === null || each.analyteId === analyteId),
  );
  return deviationOpen ? "An open deviation applies." : null;
}

/** How many analytes are accepted, and the optional whole-run assessment, in one line. */
export function qcSummary(scopes: readonly QcScope[]): string {
  const analytes = scopes.filter((scope) => scope.target.scope === "run_analyte");
  const accepted = analytes.filter(acceptanceCurrent).length;
  const run = scopes.find((scope) => scope.key === RUN_SCOPE_KEY);
  const runText = run ? ` · Whole run ${scopeStatus(run).label.toLowerCase()}` : "";
  const noun = analytes.length === 1 ? "analyte" : "analytes";
  return `${accepted} of ${analytes.length} ${noun} accepted${runText}`;
}
