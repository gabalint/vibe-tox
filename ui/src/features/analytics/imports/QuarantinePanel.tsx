import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import type { QuarantineItemView } from "@/generated/QuarantineItemView";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";
import { quarantineReasonText, quarantineStatus } from "../resultLabels";

interface QuarantinePanelProps {
  item: QuarantineItemView;
  /** The same actions as the row's menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One quarantined source row as plain text, exactly as received, and why it was held. */
export function QuarantinePanel({ item, actions, ...navigation }: QuarantinePanelProps) {
  const status = quarantineStatus(item.state);
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
        ]}
      />
    </DetailPanel>
  );
}
