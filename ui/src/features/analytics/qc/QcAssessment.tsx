import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { QcAssessmentView } from "@/generated/QcAssessmentView";
import { qcOutcomeStatus } from "../labels";

/** One QC assessment: the decision, who made it and when, what it cites, and why. */
export function QcAssessment({ item }: { item: QcAssessmentView }) {
  const status = qcOutcomeStatus(item.outcome);
  const evidence = item.evidenceObservationIds.length;
  return (
    <p>
      <StatusBadge tone={status.tone}>{status.label}</StatusBadge>{" "}
      <Timestamp value={item.assessedAt} /> by {item.assessedBy}, citing {evidence}{" "}
      {evidence === 1 ? "observation" : "observations"}
      {item.reason ? `. Reason: ${item.reason}` : ""}
    </p>
  );
}
