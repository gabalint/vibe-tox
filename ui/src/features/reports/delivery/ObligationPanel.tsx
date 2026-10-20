import type { ReactNode } from "react";
import { Link } from "react-router";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { Timestamp } from "@/components/Timestamp";
import { labelOf } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import type { ObligationView } from "../contract";
import { ATTEMPT_EVIDENCE, FAILURE_CODES, ORIGIN, sendsText } from "../labels";
import { obligationName } from "./obligationActions";

interface PanelNavigation {
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

interface ObligationPanelProps extends PanelNavigation {
  obligation: ObligationView;
  refresh?: ReactNode;
  /** The actions the engine allows on it (`useObligationActions`). */
  actions: readonly ActionItem[];
  /** Set when the list shown no longer holds it: the filter it left, in words. */
  leftFilter?: string | undefined;
}

/**
 * One delivery obligation in view mode (REQ-RPT-003, REQ-RPT-011): its status as the engine
 * words it, naming the channel and the evidence, the report it sends, and every attempt
 * recorded. Its actions are at most three, so all show as buttons, the first as the main one.
 */
export function ObligationPanel({
  obligation,
  actions,
  leftFilter,
  refresh,
  ...navigation
}: ObligationPanelProps) {
  return (
    <DetailPanel
      identifier={obligationName(obligation)}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      {refresh}
      {leftFilter && (
        <p>
          {`No longer in “${leftFilter}”. The retained record remains open; Refresh retrieves its latest status.`}
        </p>
      )}
      <Facts
        items={[
          { label: "Delivery status", value: obligation.status.label },
          {
            label: "Report",
            value: <Link to={`/reports/${obligation.reportId}`}>{obligation.reportNumber}</Link>,
          },
          { label: "Sends", value: sendsText(obligation) },
          { label: "Destination", value: obligation.destination.label },
          {
            label: "Why",
            value: obligation.originReason
              ? `${ORIGIN[obligation.origin]}: ${obligation.originReason}`
              : ORIGIN[obligation.origin],
          },
          { label: "Created", value: <Timestamp value={obligation.createdAt} /> },
          { label: "Attempts", value: <Attempts obligation={obligation} /> },
        ]}
      />
    </DetailPanel>
  );
}

/** Every attempt recorded, oldest first, each attested by the person who recorded it. */
function Attempts({ obligation }: { obligation: ObligationView }) {
  if (obligation.attempts.length === 0) return "None recorded";
  return (
    <ol aria-label="Attempts" className={styles.list}>
      {obligation.attempts.map((attempt) => (
        <li key={attempt.attemptId}>
          {ATTEMPT_EVIDENCE[attempt.evidence]}
          {attempt.failureCode && `: ${labelOf(FAILURE_CODES, attempt.failureCode)}`},{" "}
          <Timestamp value={attempt.observedAt} />, recorded by {attempt.recordedBy.displayName}.
          {attempt.note && ` ${attempt.note}`}
        </li>
      ))}
    </ol>
  );
}
