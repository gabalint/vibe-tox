import { useState } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { StatusBadge } from "@/components/StatusBadge";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { QuarantineItemView } from "@/generated/QuarantineItemView";
import type { QuarantineResolution } from "@/generated/QuarantineResolution";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";
import { quarantineReasonText, quarantineStatus } from "../resultLabels";
import { QuarantinePanel } from "./QuarantinePanel";
import { ResolveQuarantineDialog } from "./ResolveQuarantineDialog";

/** Source text is shown as plain text, exactly as received. */
const COLUMNS: readonly DataGridColumn<QuarantineItemView>[] = [
  {
    id: "record",
    header: "Record",
    identifier: true,
    cell: (row) => <span className={styles.code}>{row.recordPath}</span>,
    width: 120,
  },
  { id: "sample", header: "Sample label", cell: (row) => row.sampleLabel ?? "—", width: 140 },
  { id: "code", header: "Test code", cell: (row) => row.testCode ?? "—", width: 110 },
  { id: "value", header: "Value as received", cell: receivedValueText, width: 180, wrap: true },
  { id: "reason", header: "Why quarantined", cell: (row) => quarantineReasonText(row.reason) },
  {
    id: "state",
    header: "State",
    cell: (row) => {
      const status = quarantineStatus(row.state);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    width: 210,
  },
];

const getRowId = (row: QuarantineItemView) => row.itemId;

const ACTIONS: readonly OfferedAction<QuarantineResolution>[] = [
  { id: "accept_as_reprocessing", label: "Accept as reprocessing" },
  { id: "reject", label: "Reject row", variant: "danger" },
];

/** Interim, until the engine sends `allowedActions` on `QuarantineItemView`. */
function allowed(item: QuarantineItemView): QuarantineResolution[] {
  if (item.state !== "open") return [];
  if (item.reason !== "conflicting_duplicate") return ["reject"];
  return ["accept_as_reprocessing", "reject"];
}

interface Deciding {
  item: QuarantineItemView;
  resolution: QuarantineResolution;
}

/**
 * Rows held back from import, each needing a decision with a reason (REQ-ANA-003), opening
 * in a panel. Interim until the engine sends `allowedActions` on `QuarantineItemView`: an open
 * row offers Reject, and "Accept as reprocessing" only on the kind of row that command is for,
 * a conflicting duplicate (named by the row's `reason`). The engine still decides both.
 */
export function QuarantineItems({ items }: { items: ImportAttemptDetail["quarantine"] }) {
  const record = useOpenRecord();
  const [deciding, setDeciding] = useState<Deciding | null>(null);
  const panel = openRecordPanel(record, items, getRowId, (open) => (
    <QuarantinePanel
      item={open}
      actions={allowedActionItems(ACTIONS, allowed(open), (resolution) =>
        setDeciding({ item: open, resolution }),
      )}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (item: QuarantineItemView) => item.recordPath,
    allowed,
    onAction: (resolution, item) => setDeciding({ item, resolution }),
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <h2 className={styles.subheading}>Quarantined rows</h2>
        {items.length === 0 ? (
          <p className={styles.hint}>No rows were quarantined.</p>
        ) : (
          <DataGrid
            label="Quarantined rows"
            columns={COLUMNS}
            rows={items}
            getRowId={getRowId}
            onRowAction={(item, order) => record.open(item.itemId, order)}
            openRowId={record.openId}
            onOrderChange={record.followOrder}
            rowActions={rowActions}
            fill
          />
        )}
      </DetailLayout>
      {deciding && (
        <ResolveQuarantineDialog
          item={deciding.item}
          resolution={deciding.resolution}
          onClose={() => setDeciding(null)}
        />
      )}
    </>
  );
}
