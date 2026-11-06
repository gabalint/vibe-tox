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
import { RetryImportDialog } from "./RetryImportDialog";

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

/** A decision on the row, or a retry of the whole received message once its cause is fixed. */
type RowAction = QuarantineResolution | "retry_import";

const ACTIONS: readonly OfferedAction<RowAction>[] = [
  { id: "retry_import", label: "Retry import" },
  { id: "accept_as_reprocessing", label: "Accept as reprocessing" },
  { id: "reject", label: "Reject row", variant: "danger" },
];

/** Interim, until the engine sends `allowedActions` on `QuarantineItemView`. */
function allowed(item: QuarantineItemView): RowAction[] {
  if (item.state !== "open") return [];
  if (item.reason !== "conflicting_duplicate") return ["retry_import", "reject"];
  return ["retry_import", "accept_as_reprocessing", "reject"];
}

interface Deciding {
  item: QuarantineItemView;
  resolution: QuarantineResolution;
}

/**
 * Rows held back from import, each needing a decision with a reason (REQ-ANA-003), opening
 * in a panel that links to where its cause is fixed. Interim until the engine sends
 * `allowedActions` on `QuarantineItemView`: an open row offers Retry import (of the whole
 * received message, once the cause is fixed) and Reject, and "Accept as reprocessing" only on
 * the kind of row that command is for, a conflicting duplicate (named by the row's `reason`).
 * The engine still decides each.
 * When the whole message or file was refused, its rows count as quarantined without a row
 * each, so the section points to the failure instead of saying none were held.
 */
export function QuarantineItems({ attempt }: { attempt: ImportAttemptDetail }) {
  const items = attempt.quarantine;
  const record = useOpenRecord();
  const [deciding, setDeciding] = useState<Deciding | null>(null);
  const [isRetrying, setRetrying] = useState(false);
  const act = (action: RowAction, item: QuarantineItemView) =>
    action === "retry_import" ? setRetrying(true) : setDeciding({ item, resolution: action });
  const panel = openRecordPanel(record, items, getRowId, (open) => (
    <QuarantinePanel
      item={open}
      attempt={attempt}
      actions={allowedActionItems(ACTIONS, allowed(open), (action) => act(action, open))}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (item: QuarantineItemView) => item.recordPath,
    allowed,
    onAction: act,
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <h2 className={styles.subheading}>Quarantined rows</h2>
        {items.length === 0 ? (
          <p className={styles.hint}>{emptyText(attempt)}</p>
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
      {isRetrying && (
        <RetryImportDialog receiptId={attempt.receiptId} onClose={() => setRetrying(false)} />
      )}
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

function emptyText(attempt: ImportAttemptDetail): string {
  const count = attempt.rowsQuarantined;
  if (attempt.failureCode === null || count === 0) return "No rows were quarantined.";
  const [whole, failure] = attempt.file === null ? ["message", "Message"] : ["file", "File"];
  return `The whole ${whole} was held: ${count} ${count === 1 ? "row" : "rows"}. See ${failure} failure above.`;
}
