import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { DeviationView } from "@/generated/DeviationView";
import { deviationStatus } from "./deviations";

interface DeviationPanelProps {
  deviation: DeviationView;
  /** What it applies to and the analyte it is limited to, in words. */
  appliesTo: string;
  analyte: string;
  /** The same actions as the deviation's row menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One deviation as recorded: what happened, why it was opened, and how it was closed. */
export function DeviationPanel(props: DeviationPanelProps) {
  const { deviation, appliesTo, analyte, actions, ...navigation } = props;
  const status = deviationStatus(deviation);
  return (
    <DetailPanel
      identifier={deviation.description}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Applies to", value: appliesTo },
          { label: "Analyte", value: analyte },
          { label: "Reason", value: deviation.reason },
          {
            label: "Opened",
            value: (
              <>
                <Timestamp value={deviation.openedAt} /> by {deviation.openedBy}
              </>
            ),
          },
          ...closingFacts(deviation),
        ]}
      />
    </DetailPanel>
  );
}

function closingFacts(deviation: DeviationView) {
  if (deviation.closedAt === null) return [];
  return [
    {
      label: "Closed",
      value: (
        <>
          <Timestamp value={deviation.closedAt} /> by {deviation.closedBy ?? "Not recorded"}
        </>
      ),
    },
    { label: "Disposition", value: deviation.disposition ?? "Not recorded" },
    { label: "Reason for closing", value: deviation.closeReason ?? "Not recorded" },
  ];
}
