import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { ListCount } from "@/components/ListLimit";
import { Toolbar } from "@/components/Toolbar";
import type { ExecutionView } from "@/generated/ExecutionView";
import type { RunDetail } from "@/generated/RunDetail";
import type { RunPositionView } from "@/generated/RunPositionView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { labelOf } from "@/lib/choices";
import { useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import styles from "@/styles/layout.module.css";
import { PERFORM_ANALYSIS } from "../contract";
import { ATTEMPT_KINDS, POSITION_ROLES } from "../labels";
import { ExecutionState } from "./ExecutionState";
import { FailExecutionDialog } from "./FailExecutionDialog";
import { PatientPositionForm } from "./PatientPositionForm";
import { PositionPanel } from "./PositionPanel";
import { QcPositionDialog } from "./QcPositionDialog";
import { RepeatPositionDialog } from "./RepeatPositionDialog";

const COLUMNS: readonly DataGridColumn<RunPositionView>[] = [
  {
    id: "label",
    header: "Position",
    identifier: true,
    cell: (row) => row.label,
    sortValue: (row) => row.label,
    width: 120,
  },
  { id: "role", header: "Role", cell: (row) => labelOf(POSITION_ROLES, row.role), width: 120 },
  {
    id: "sample",
    header: "Sample label",
    cell: (row) => <span className={styles.code}>{row.sampleLabel}</span>,
    sortValue: (row) => row.sampleLabel,
  },
  {
    id: "attempt",
    header: "Attempt",
    cell: (row) =>
      row.execution === null ? "—" : labelOf(ATTEMPT_KINDS, row.execution.attemptKind),
    width: 140,
  },
  {
    id: "execution",
    header: "Execution",
    cell: (row) => <ExecutionState execution={row.execution} />,
  },
  {
    id: "analytes",
    header: "Expected analytes",
    cell: (row) => (row.execution === null ? "—" : row.execution.expectedAnalytes),
    align: "end",
    width: 160,
  },
];

const getRowId = (row: RunPositionView) => row.positionId;

const ACTIONS: readonly OfferedAction<"fail_execution">[] = [
  { id: "fail_execution", label: "Mark attempt failed", variant: "danger" },
];

/** A patient attempt's actions are its execution's; a control, calibrator or blank has none. */
const allowed = (position: RunPositionView) => position.execution?.allowedActions ?? [];

type PositionDialog =
  | { kind: "qc" }
  | { kind: "repeat"; position: RunPositionView }
  | { kind: "fail"; position: RunPositionView; execution: ExecutionView }
  | null;

/**
 * The run's positions in the engine's order, with each patient attempt's kind and state,
 * opening in a panel, with the add-sample scan above them. A patient attempt the engine
 * allows to fail (`fail_execution`) is marked failed from the panel or the row menu.
 */
export function Positions({ run }: { run: RunDetail }) {
  const record = useOpenRecord();
  const canPerform = useHasCapability(PERFORM_ANALYSIS);
  const [dialog, setDialog] = useState<PositionDialog>(null);
  const close = () => setDialog(null);
  const fail = (position: RunPositionView) => {
    const { execution } = position;
    if (execution) setDialog({ kind: "fail", position, execution });
  };
  const panel = openRecordPanel(record, run.positions, getRowId, (open) => (
    <PositionPanel
      position={open}
      lots={run.lots}
      actions={allowedActionItems(ACTIONS, allowed(open), () => fail(open))}
      onRepeat={
        canPerform && open.execution
          ? () => setDialog({ kind: "repeat", position: open })
          : undefined
      }
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: RunPositionView) => row.label,
    allowed,
    onAction: (_, row) => fail(row),
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={<ListCount shown={run.positions.length} noun="positions" one="position" />}
        >
          <h2 className={styles.subheading}>Positions</h2>
          {run.allowedActions.includes("add_qc_position") && (
            <Button onPress={() => setDialog({ kind: "qc" })}>
              Add control, calibrator or blank
            </Button>
          )}
        </Toolbar>
        {run.allowedActions.includes("add_patient_position") && <PatientPositionForm run={run} />}
        {run.positions.length === 0 ? (
          <EmptyState
            title="No positions yet"
            description="No sample, control, calibrator or blank is on this run."
          />
        ) : (
          <DataGrid
            label="Positions"
            columns={COLUMNS}
            rows={run.positions}
            getRowId={getRowId}
            onRowAction={(row, order) => record.open(row.positionId, order)}
            openRowId={record.openId}
            onOrderChange={record.followOrder}
            rowActions={rowActions}
            fill
          />
        )}
      </DetailLayout>
      {dialog?.kind === "repeat" && (
        <RepeatPositionDialog run={run} position={dialog.position} onClose={close} />
      )}
      {dialog?.kind === "qc" && <QcPositionDialog run={run} onClose={close} />}
      {dialog?.kind === "fail" && (
        <FailExecutionDialog
          position={dialog.position}
          execution={dialog.execution}
          onClose={close}
        />
      )}
    </>
  );
}
