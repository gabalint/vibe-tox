import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Timestamp } from "@/components/Timestamp";
import type { PrepBatchSummary } from "@/generated/PrepBatchSummary";
import { usePrepBatches } from "./PrepBatchChoice";

const COLUMNS: readonly DataGridColumn<PrepBatchSummary>[] = [
  {
    id: "created",
    header: "Started",
    identifier: true,
    cell: (row) => <Timestamp value={row.createdAt} />,
  },
  { id: "batch", header: "Batch", cell: (row) => row.prepBatchId },
  { id: "state", header: "State", cell: (row) => (row.state === "open" ? "Open" : "Completed") },
  { id: "steps", header: "Steps", cell: (row) => row.stepCount, align: "end" },
];

export function PrepBatchDirectory({
  methodVersionId,
  onOpen,
}: {
  methodVersionId: string;
  onOpen: (id: string) => void;
}) {
  const batches = usePrepBatches(methodVersionId);
  return (
    <>
      <RefreshControl query={batches} />
      <QueryContent
        query={batches}
        what="preparation batches"
        empty={{
          when: (data) => data.pages.flat().length === 0,
          title: "No preparation batches",
          description:
            "Start a batch for this method, then return here to continue its recorded steps.",
        }}
      >
        {(data) => (
          <DataGrid
            label="Preparation batches"
            columns={COLUMNS}
            rows={data.pages.flat()}
            getRowId={(row) => row.prepBatchId}
            onRowAction={(row) => onOpen(row.prepBatchId)}
          />
        )}
      </QueryContent>
      {batches.hasNextPage && (
        <Button isPending={batches.isFetchingNextPage} onPress={() => void batches.fetchNextPage()}>
          Load more preparation batches
        </Button>
      )}
    </>
  );
}
