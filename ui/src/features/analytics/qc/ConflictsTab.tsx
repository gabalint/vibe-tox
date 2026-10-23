import { useMemo, useState } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { ListCount } from "@/components/ListLimit";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { QcConflictResolution } from "@/generated/QcConflictResolution";
import type { RunQcView } from "@/generated/RunQcView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { gateLabel } from "@/lib/errors";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { usePersonText } from "@/lib/person";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";
import { ConflictPanel } from "./ConflictPanel";
import { CONFLICT_STATES, type ConflictRow, conflictName, conflictRows } from "./conflictRows";
import { ResolveConflictDialog } from "./ResolveConflictDialog";

const COLUMNS: readonly DataGridColumn<ConflictRow>[] = [
  {
    id: "analyte",
    header: "Analyte",
    identifier: true,
    cell: (row) => row.analyteCode,
    width: 120,
  },
  {
    id: "code",
    header: "Test code",
    cell: (row) => <span className={styles.code}>{row.sourceTestCode}</span>,
    width: 110,
  },
  { id: "stored", header: "Stored observation", cell: (row) => row.stored, wrap: true },
  { id: "received", header: "Changed value as received", cell: receivedValueText, wrap: true },
  { id: "flags", header: "Flags", cell: (row) => row.flagsText ?? "—", width: 90 },
  { id: "status", header: "Status", cell: (row) => row.statusText ?? "—", width: 90 },
  {
    id: "state",
    header: "State",
    cell: (row) => (
      <StatusBadge tone={CONFLICT_STATES[row.state].tone}>
        {CONFLICT_STATES[row.state].label}
      </StatusBadge>
    ),
    width: 190,
  },
  {
    id: "decision",
    header: "Decision",
    cell: (row) =>
      row.resolvedAt ? (
        <>
          {row.reason ?? ""} (<DecidedBy row={row} />, <Timestamp value={row.resolvedAt} />)
        </>
      ) : (
        <>
          Received <Timestamp value={row.recordedAt} />
        </>
      ),
  },
];

/** Who decided a conflict, by name; the column definitions cannot call hooks themselves. */
function DecidedBy({ row }: { row: ConflictRow }) {
  const person = usePersonText();
  return row.resolvedBy ? person(row.resolvedBy, row.resolvedByDisplay) : "";
}

const getRowId = (row: ConflictRow) => row.conflictId;
const ACTIONS: readonly OfferedAction<QcConflictResolution>[] = [
  { id: "accept_as_correction", label: "Accept as correction" },
  { id: "reject", label: "Reject changed row", variant: "danger" },
];
/** Interim: an open row offers both decisions, as before; the engine checks each. */
const allowed = (conflict: ConflictRow) =>
  conflict.state === "open" ? ACTIONS.map(({ id }) => id) : [];

interface Deciding {
  conflict: ConflictRow;
  resolution: QcConflictResolution;
}

/**
 * Changed QC rows the analyzer's registration cannot classify (REQ-ANA-004, REQ-ANA-005).
 * While one is open, results for its analyte on this run cannot be reviewed. A row opens in
 * the panel to be accepted as a correction or rejected, each with a reason. The rows carry
 * no `allowedActions` yet: an open row offers both decisions, and the engine checks each.
 */
export function ConflictsTab({ view }: { view: RunQcView }) {
  const rows = useMemo(() => conflictRows(view), [view]);
  const record = useOpenRecord();
  const [deciding, setDeciding] = useState<Deciding | null>(null);
  if (rows.length === 0) {
    return (
      <EmptyState
        title="No changed QC rows"
        description="The analyzer has not sent a different value for QC already received."
      />
    );
  }
  const panel = openRecordPanel(record, rows, getRowId, (open) => (
    <ConflictPanel
      conflict={open}
      actions={allowedActionItems(ACTIONS, allowed(open), (resolution) =>
        setDeciding({ conflict: open, resolution }),
      )}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: conflictName,
    allowed,
    onAction: (resolution, conflict) => setDeciding({ conflict, resolution }),
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <p>
          {`The analyzer sent a different value for QC already received. Review of the affected results is blocked ("${gateLabel("qc_conflict")}") until each row is decided.`}
        </p>
        <Toolbar
          variant="collection"
          count={<ListCount shown={rows.length} noun="changed QC rows" one="changed QC row" />}
        />
        <DataGrid
          label="Changed QC rows"
          columns={COLUMNS}
          rows={rows}
          getRowId={getRowId}
          onRowAction={(row, order) => record.open(row.conflictId, order)}
          openRowId={record.openId}
          onOrderChange={record.followOrder}
          rowActions={rowActions}
          fill
        />
      </DetailLayout>
      {deciding && (
        <ResolveConflictDialog
          conflict={deciding.conflict}
          resolution={deciding.resolution}
          onClose={() => setDeciding(null)}
        />
      )}
    </>
  );
}
