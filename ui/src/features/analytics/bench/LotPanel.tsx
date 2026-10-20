import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { LotView } from "@/generated/LotView";
import { labelOf } from "@/lib/choices";
import { lotStatus, MATERIAL_KINDS } from "../labels";

interface LotPanelProps {
  lot: LotView;
  /** The same actions as the lot's row menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One lot as registered from its packaging, with its expiry as the engine derives it. */
export function LotPanel({ lot, actions, ...navigation }: LotPanelProps) {
  const status = lotStatus(lot);
  return (
    <DetailPanel
      identifier={`Lot ${lot.lotNumber}`}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Material", value: labelOf(MATERIAL_KINDS, lot.materialKind) },
          { label: "Product", value: lot.productName },
          { label: "Lot number", value: lot.lotNumber },
          { label: "Level", value: lot.levelText ?? "Not recorded" },
          { label: "Sample label", value: lot.sampleLabel ?? "Not recorded" },
          { label: "Expires on", value: lot.expiresOn },
          { label: "Registered", value: <Timestamp value={lot.recordedAt} /> },
        ]}
      />
    </DetailPanel>
  );
}
