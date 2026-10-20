import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { instrumentStatus, sourceText } from "../labels";
import { InstrumentSources } from "./InstrumentSources";

interface InstrumentPanelProps {
  instrument: InstrumentReadinessView;
  /** The same actions as the instrument's row menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One instrument's service status and its registered result sources (REQ-ANA-003, -007). */
export function InstrumentPanel({ instrument, actions, ...navigation }: InstrumentPanelProps) {
  const status = instrumentStatus(instrument.status);
  return (
    <DetailPanel
      identifier={instrument.name}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          {
            label: "Status since",
            value:
              instrument.since === null ? "Not recorded" : <Timestamp value={instrument.since} />,
          },
          { label: "Approved result source", value: sourceText(instrument) },
        ]}
      />
      <InstrumentSources instrument={instrument} />
    </DetailPanel>
  );
}
