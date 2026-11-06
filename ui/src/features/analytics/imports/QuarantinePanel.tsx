import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { QuarantineItemView } from "@/generated/QuarantineItemView";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";
import { quarantineReasonText, quarantineStatus } from "../resultLabels";
import { useRowFixFacts } from "./fixLinks";

interface QuarantinePanelProps {
  item: QuarantineItemView;
  /** The attempt the row belongs to: its instrument and source name where to fix it. */
  attempt: ImportAttemptDetail;
  /** The same actions as the row's menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/**
 * One quarantined source row as plain text, exactly as received, why it was held and where
 * that is fixed.
 */
export function QuarantinePanel({ item, attempt, actions, ...navigation }: QuarantinePanelProps) {
  const status = quarantineStatus(item.state);
  const fix = useRowFixFacts(item, attempt);
  return (
    <DetailPanel
      identifier={`Row ${item.recordPath}`}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Record", value: <span className={styles.code}>{item.recordPath}</span> },
          { label: "Sample label", value: item.sampleLabel ?? "Not reported" },
          { label: "Test code", value: item.testCode ?? "Not reported" },
          { label: "Value as received", value: receivedValueText(item) },
          { label: "Why quarantined", value: quarantineReasonText(item.reason) },
          ...fix,
        ]}
      />
    </DetailPanel>
  );
}
