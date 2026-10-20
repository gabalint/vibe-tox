import type { ReactNode } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount, ListLimit } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { Toolbar } from "@/components/Toolbar";
import type { WorkQueueItem } from "@/generated/WorkQueueItem";
import { QueueScanControls } from "./QueueScanControls";
import type { useWorkQueueItems } from "./useWorkQueueItems";

interface WorkQueueListProps {
  query: ReturnType<typeof useWorkQueueItems>;
  total?: { count: number; capped: boolean };
  label: string;
  controls: ReactNode;
  columns: readonly DataGridColumn<WorkQueueItem>[];
  getRowId: (row: WorkQueueItem) => string;
  onOpen: (row: WorkQueueItem, order: readonly string[]) => void;
}

/** The release queue and operational queues share count evidence, order and list states. */
export function WorkQueueList({
  query,
  total,
  label,
  controls,
  columns,
  getRowId,
  onOpen,
}: WorkQueueListProps) {
  const rows = query.data ? query.rows : undefined;
  return (
    <>
      <Toolbar
        variant="collection"
        count={rows && <ListCount shown={rows.length} noun="items" one="item" total={total} />}
      >
        {controls}
      </Toolbar>
      {rows && (
        <ListLimit
          variant="summary"
          shown={rows.length}
          end="oldest"
          total={total}
          hasMore={query.hasNextPage}
        />
      )}
      <QueryContent
        query={query}
        what="queue items"
        loadingRows={8}
        empty={{
          when: () => rows?.length === 0 && !query.hasNextPage,
          title: "Nothing waiting",
          description: "No item is in this queue.",
        }}
      >
        {() => (
          <DataGrid
            label={label}
            columns={columns}
            rows={rows ?? []}
            getRowId={getRowId}
            onRowAction={onOpen}
            fill
          />
        )}
      </QueryContent>
      {query.data && <QueueScanControls query={query} />}
    </>
  );
}
