import { useState } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { FindCount } from "@/components/FindField";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { HandoffItemView } from "@/generated/HandoffItemView";
import { allowedActionItems, allowedRowActions } from "@/lib/allowedActions";
import { checked, labelOf } from "@/lib/choices";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import styles from "@/styles/layout.module.css";
import { ITEM_KINDS, itemStatus } from "../labels";
import { derivationText, HandoffItemPanel } from "./HandoffItemPanel";
import { RecordOutcomeDialog } from "./RecordOutcomeDialog";

function outcomeText({ outcome }: HandoffItemView) {
  if (outcome === null) return "None yet";
  const detail = outcome.reason ?? outcome.externalReference;
  return (
    <>
      {detail ? `${detail}, ` : ""}
      <Timestamp value={outcome.recordedAt} />
    </>
  );
}

const COLUMNS: readonly DataGridColumn<HandoffItemView>[] = [
  {
    id: "derivation",
    header: "Derivation",
    cell: derivationText,
    sortValue: (item) => item.derivationNo,
    width: 140,
    identifier: true,
  },
  { id: "item", header: "Item", cell: (item) => item.itemId },
  { id: "kind", header: "Kind", cell: (item) => labelOf(ITEM_KINDS, item.kind), width: 120 },
  { id: "adjusts", header: "Adjusts item", cell: (item) => item.adjustsItemId ?? "None" },
  {
    id: "status",
    header: "Status",
    cell: (item) => {
      const status = itemStatus(item.status);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    width: 180,
  },
  { id: "outcome", header: "Outcome details", cell: outcomeText },
];

const ACTIONS = [
  { id: "record_handoff_outcome", label: "Record outcome" },
  { id: "record_handoff_followup", label: "Record follow-up outcome" },
];

const itemId = (item: HandoffItemView) => item.itemId;

/**
 * The handoff's items, each acknowledged, rejected or still awaiting the partner's answer
 * (REQ-BIL-002). An item opens in the side panel (`?open=`), which links to its derivation;
 * the partner's outcome is recorded from the panel or the row menu when the engine allows it.
 */
export function HandoffItems({ items }: { items: readonly HandoffItemView[] }) {
  const record = useOpenRecord();
  const [recording, setRecording] = useState<{ item: HandoffItemView; followup: boolean } | null>(
    null,
  );
  const recordOutcome = (action: string, item: HandoffItemView) =>
    setRecording({ item, followup: action === "record_handoff_followup" });
  const panel = openRecordPanel(record, items, itemId, (open) => (
    <HandoffItemPanel
      item={open}
      actions={allowedActionItems(ACTIONS, open.allowedActions, (action) =>
        recordOutcome(action, open),
      )}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: derivationText,
    allowed: (item) => item.allowedActions,
    onAction: recordOutcome,
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={
            <FindCount
              shown={items.length}
              total={items.length}
              noun="handoff items"
              one="handoff item"
            />
          }
        >
          <h2 className={styles.subheading}>Items</h2>
        </Toolbar>
        <DataGrid
          label="Handoff items"
          columns={COLUMNS}
          rows={items}
          getRowId={itemId}
          onRowAction={(item, order) => record.open(item.itemId, order)}
          openRowId={record.openId}
          onOrderChange={record.followOrder}
          rowActions={rowActions}
          fill
        />
      </DetailLayout>
      {recording &&
        (recording.followup ? (
          <RecordOutcomeDialog
            itemId={recording.item.itemId}
            followup
            revision={checked(recording.item.outcomeRevision ?? null)}
            onClose={() => setRecording(null)}
          />
        ) : (
          <RecordOutcomeDialog itemId={recording.item.itemId} onClose={() => setRecording(null)} />
        ))}
    </>
  );
}
