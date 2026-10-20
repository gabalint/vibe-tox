import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { ListCount } from "@/components/ListLimit";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { DeviationView } from "@/generated/DeviationView";
import type { RunQcView } from "@/generated/RunQcView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { CloseDeviationDialog } from "./CloseDeviationDialog";
import { DeviationPanel } from "./DeviationPanel";
import { analyteText, deviationStatus, scopeText } from "./deviations";
import { OpenDeviationDialog, type QcTargets, qcTargets } from "./OpenDeviationDialog";

function deviationColumns(targets: QcTargets): DataGridColumn<DeviationView>[] {
  return [
    {
      id: "description",
      header: "Description",
      identifier: true,
      cell: (row) => row.description,
    },
    { id: "scope", header: "Applies to", cell: (row) => scopeText(row, targets), width: 200 },
    {
      id: "state",
      header: "State",
      cell: (row) => {
        const status = deviationStatus(row);
        return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
      },
      width: 110,
    },
    {
      id: "opened",
      header: "Opened",
      cell: (row) => (
        <>
          <Timestamp value={row.openedAt} /> by {row.openedBy}
        </>
      ),
    },
    { id: "disposition", header: "Disposition", cell: (row) => row.disposition ?? "—" },
  ];
}

const getRowId = (row: DeviationView) => row.deviationId;

const ACTIONS: readonly OfferedAction<"close_deviation">[] = [
  { id: "close_deviation", label: "Close deviation" },
];
/** Interim: an open deviation may be closed, as before; the engine checks it. */
const allowed = (deviation: DeviationView) =>
  deviation.state === "open" ? ["close_deviation"] : [];

type DeviationDialog = { kind: "open" } | { kind: "close"; deviation: DeviationView } | null;

/**
 * Deviations recorded against this run, its positions or its lots (REQ-ANA-004), opening in
 * a panel. Deviations carry no `allowedActions` yet: anyone on this screen may open one, and
 * an open one may be closed with its disposition; the engine checks both.
 */
export function DeviationsTab({
  view,
  lotName,
}: {
  view: RunQcView;
  lotName: (lotId: string | null) => string;
}) {
  const targets = useMemo(() => qcTargets(view, lotName), [view, lotName]);
  const columns = useMemo(() => deviationColumns(targets), [targets]);
  const record = useOpenRecord();
  const [dialog, setDialog] = useState<DeviationDialog>(null);
  const close = () => setDialog(null);
  const closeDeviation = (deviation: DeviationView) => setDialog({ kind: "close", deviation });
  const panel = openRecordPanel(record, view.deviations, getRowId, (open) => (
    <DeviationPanel
      deviation={open}
      appliesTo={scopeText(open, targets)}
      analyte={analyteText(open, targets)}
      actions={allowedActionItems(ACTIONS, allowed(open), () => closeDeviation(open))}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: DeviationView) => row.description,
    allowed,
    onAction: (_, row) => closeDeviation(row),
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={<ListCount shown={view.deviations.length} noun="deviations" one="deviation" />}
        >
          <Button onPress={() => setDialog({ kind: "open" })}>Open deviation</Button>
        </Toolbar>
        {view.deviations.length === 0 ? (
          <EmptyState title="No deviations recorded" />
        ) : (
          <DataGrid
            label="Deviations"
            columns={columns}
            rows={view.deviations}
            getRowId={getRowId}
            onRowAction={(row, order) => record.open(row.deviationId, order)}
            openRowId={record.openId}
            onOrderChange={record.followOrder}
            rowActions={rowActions}
            fill
          />
        )}
      </DetailLayout>
      {dialog?.kind === "open" && (
        <OpenDeviationDialog runId={view.runId} targets={targets} onClose={close} />
      )}
      {dialog?.kind === "close" && (
        <CloseDeviationDialog deviation={dialog.deviation} onClose={close} />
      )}
    </>
  );
}
