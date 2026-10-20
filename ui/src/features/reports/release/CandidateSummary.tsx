import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import styles from "@/styles/layout.module.css";
import type { CandidateView } from "../contract";
import { CANDIDATE_STATE, COMPLETENESS, revisionText } from "../labels";

/** A candidate's state, what it proposes and when it expires. */
export function CandidateSummary({ candidate }: { candidate: CandidateView }) {
  const state = CANDIDATE_STATE[candidate.state];
  return (
    <Facts
      layout="compact"
      items={[
        {
          label: "State",
          value: (
            <>
              <StatusBadge tone={state.tone}>{state.label}</StatusBadge>{" "}
              {candidate.stale && (
                <StatusBadge tone="warning">Content changed since preparation</StatusBadge>
              )}
            </>
          ),
        },
        {
          label: "Proposed",
          value: `${revisionText(candidate.proposedRevisionNo, candidate.relationship)}, ${COMPLETENESS[candidate.completeness].label.toLowerCase()}`,
        },
        ...(candidate.supersedesRevisionNo === null
          ? []
          : [{ label: "Follows revision", value: String(candidate.supersedesRevisionNo) }]),
        ...(candidate.amendmentReason
          ? [{ label: "Reason for correction", value: candidate.amendmentReason }]
          : []),
        { label: "Signatory", value: candidate.signatory.displayName },
        { label: "Prepared on", value: <Timestamp value={candidate.preparedAt} /> },
        { label: "Expires", value: <Timestamp value={candidate.expiresAt} /> },
        {
          label: "Content digest",
          value: <span className={styles.code}>{candidate.contentSha256}</span>,
        },
      ]}
    />
  );
}
