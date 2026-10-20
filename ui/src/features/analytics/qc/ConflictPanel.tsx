import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";
import { CONFLICT_STATES, type ConflictRow, conflictName } from "./conflictRows";

interface ConflictPanelProps {
  conflict: ConflictRow;
  /** The same actions as the row's menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One changed QC row, exactly as received, beside the stored observation it contradicts. */
export function ConflictPanel({ conflict, actions, ...navigation }: ConflictPanelProps) {
  const state = CONFLICT_STATES[conflict.state];
  return (
    <DetailPanel
      identifier={conflictName(conflict)}
      status={<StatusBadge tone={state.tone}>{state.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Analyte", value: conflict.analyteCode },
          {
            label: "Test code",
            value: <span className={styles.code}>{conflict.sourceTestCode}</span>,
          },
          { label: "Stored observation", value: conflict.stored },
          { label: "Changed value as received", value: receivedValueText(conflict) },
          { label: "Flags as received", value: conflict.flagsText ?? "Not reported" },
          { label: "Status as received", value: conflict.statusText ?? "Not reported" },
          { label: "Received", value: <Timestamp value={conflict.recordedAt} /> },
          ...decisionFacts(conflict),
        ]}
      />
    </DetailPanel>
  );
}

function decisionFacts(conflict: ConflictRow) {
  if (conflict.resolvedAt === null) return [];
  return [
    {
      label: "Decided",
      value: (
        <>
          <Timestamp value={conflict.resolvedAt} /> by {conflict.resolvedBy ?? "Not recorded"}
        </>
      ),
    },
    { label: "Reason", value: conflict.reason ?? "Not recorded" },
  ];
}
