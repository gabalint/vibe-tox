import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { Timestamp } from "@/components/Timestamp";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import styles from "@/styles/layout.module.css";
import { Person } from "../Person";
import { ApprovalBadge } from "./approvalColumns";

/** The facts of an approval, which the version panel shows too. */
export function approvalFacts(approval: UpdateApprovalView) {
  return [
    { label: "Risk class, as published", value: approval.riskClassText },
    { label: "Package file", value: approval.packageFileName },
    {
      label: "Package SHA-256",
      value: <span className={styles.code}>{approval.packageSha256}</span>,
    },
    {
      label: "Pre-update backup run",
      value: <span className={styles.code}>{approval.preUpdateRunId}</span>,
    },
  ];
}

interface ApprovalPanelProps {
  approval: UpdateApprovalView;
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One update approval in view mode. */
export function ApprovalPanel({ approval, actions, ...navigation }: ApprovalPanelProps) {
  const { withdrawn } = approval;
  return (
    <DetailPanel
      identifier={`Approval of version ${approval.version}`}
      status={<ApprovalBadge approval={approval} />}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Version", value: approval.version },
          ...approvalFacts(approval),
          {
            label: "Approved",
            value: (
              <>
                <Timestamp value={approval.approvedAt} /> by{" "}
                <Person id={approval.approvedBy} display={approval.approvedByDisplay} />
              </>
            ),
          },
          {
            label: "Withdrawn",
            value: withdrawn ? (
              <>
                <Timestamp value={withdrawn.at} /> by{" "}
                <Person id={withdrawn.by} display={withdrawn.byDisplay} />
              </>
            ) : (
              "No"
            ),
          },
        ]}
      />
    </DetailPanel>
  );
}
