import { useMemo } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { LotView } from "@/generated/LotView";
import type { PrepBatchView } from "@/generated/PrepBatchView";
import type { PrepStepView } from "@/generated/PrepStepView";
import { useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import styles from "@/styles/layout.module.css";
import { LIST_LOTS } from "../contract";
import { lotText } from "../labels";

function stepColumns(
  lots: readonly LotView[],
  person: (userId: string, display?: string) => string,
): DataGridColumn<PrepStepView>[] {
  const lotName = (id: string | null) => {
    const lot = lots.find((candidate) => candidate.lotId === id);
    return lot ? lotText(lot) : (id ?? "—");
  };
  return [
    { id: "seq", header: "Step", cell: (row) => row.seq, width: 70 },
    { id: "code", header: "Step code", cell: (row) => row.stepCode, width: 160 },
    { id: "lot", header: "Lot", cell: (row) => lotName(row.lotId) },
    { id: "note", header: "Note", cell: (row) => row.note ?? "—" },
    {
      id: "by",
      header: "Performed",
      cell: (row) => (
        <>
          <Timestamp value={row.performedAt} /> by {person(row.performedBy, row.performedByDisplay)}
        </>
      ),
    },
  ];
}

export function PrepBatchDetail({ batch }: { batch: PrepBatchView }) {
  const formatTime = useFormatTime();
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: false } });
  const person = usePersonText();
  const columns = useMemo(() => stepColumns(lots.data ?? [], person), [lots.data, person]);
  const completed = batch.completedAt
    ? `${formatTime(batch.completedAt)} by ${batch.completedBy ? person(batch.completedBy, batch.completedByDisplay) : ""}`
    : "Not yet";
  return (
    <>
      <Facts
        items={[
          { label: "Batch", value: <span className={styles.code}>{batch.prepBatchId}</span> },
          {
            label: "State",
            value:
              batch.state === "open" ? (
                <StatusBadge tone="neutral">Open</StatusBadge>
              ) : (
                <StatusBadge tone="neutral">Completed</StatusBadge>
              ),
          },
          {
            label: "Started",
            value: `${formatTime(batch.createdAt)} by ${person(batch.createdBy, batch.createdByDisplay)}`,
          },
          { label: "Completed", value: completed },
        ]}
      />
      {batch.steps.length === 0 ? (
        <p className={styles.hint}>No steps recorded yet.</p>
      ) : (
        <DataGrid
          label="Preparation steps"
          columns={columns}
          rows={batch.steps}
          getRowId={(row) => row.stepId}
        />
      )}
    </>
  );
}
