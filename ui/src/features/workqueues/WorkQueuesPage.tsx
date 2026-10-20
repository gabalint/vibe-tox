import { useNavigate } from "react-router";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Toolbar } from "@/components/Toolbar";
import { useCommandQuery } from "@/lib/commands";
import { Age } from "./Age";
import { LIST_WORK_QUEUES, type WorkQueueSummary } from "./contract";

const queuePath = (row: WorkQueueSummary) => `/work-queues/${row.queue}`;

const COLUMNS: readonly DataGridColumn<WorkQueueSummary>[] = [
  {
    id: "queue",
    header: "Queue",
    cell: (row) => row.label,
    sortValue: (row) => row.label,
    identifier: true,
  },
  {
    id: "count",
    header: "Items",
    cell: (row) =>
      row.count === null ? "Open to check" : row.countCapped ? `${row.count} or more` : row.count,
    sortValue: (row) => row.count ?? -1,
    align: "end",
    width: 110,
  },
  {
    id: "oldest",
    header: "Oldest item waiting",
    cell: (row) => <Age since={row.oldestSince} />,
    sortValue: (row) => row.oldestSince ?? "",
    width: 320,
  },
  { id: "next", header: "Next action", cell: (row) => row.nextAction },
];

/**
 * The work waiting across the laboratory (REQ-OPS-004), one row per queue the signed-in
 * person may work: how many items wait, how long the oldest has waited, and what to do next.
 * Other people change the queues, so the time of the last read shows beside Refresh.
 */
export function WorkQueuesPage() {
  const navigate = useNavigate();
  const queues = useCommandQuery<WorkQueueSummary[]>(LIST_WORK_QUEUES);
  return (
    <Page layout="collection">
      <PageHeader title="Work queues" />
      <Toolbar
        variant="collection"
        count={queues.data && <ListCount shown={queues.data.length} noun="queues" one="queue" />}
      >
        <RefreshControl query={queues} />
      </Toolbar>
      <QueryContent
        query={queues}
        what="work queues"
        loadingRows={8}
        empty={{
          when: (rows) => rows.length === 0,
          title: "No work queues",
          description: "None of your capabilities has a work queue.",
        }}
      >
        {(rows) => (
          <DataGrid
            label="Work queues"
            columns={COLUMNS}
            rows={rows}
            getRowId={(row) => row.queue}
            onRowAction={(row) => navigate(queuePath(row))}
            fill
          />
        )}
      </QueryContent>
    </Page>
  );
}
