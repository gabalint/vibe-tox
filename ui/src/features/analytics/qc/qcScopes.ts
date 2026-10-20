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
      key: "run",
      label: "Whole run",
      title: "Whole run",
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

/** The scope's current assessment, or that it has none yet. */
export function scopeStatus(scope: QcScope): Status {
  const { current } = scope.assessments;
  return current === null
    ? { label: "Not assessed", tone: "neutral" }
    : qcOutcomeStatus(current.outcome);
}
