import type { DataGridColumn } from "@/components/DataGrid";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Timestamp } from "@/components/Timestamp";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import { useWorkQueueItems } from "@/lib/useWorkQueueItems";
import { WorkQueueList } from "@/lib/WorkQueueList";
import {
  LIST_LIMIT,
  LIST_WORK_QUEUES,
  RELEASE_REPORTS,
  type WorkQueueItem,
  type WorkQueueSummary,
} from "../contract";
import { revisionPath, workspacePath } from "../paths";

const COLUMNS: readonly DataGridColumn<WorkQueueItem>[] = [
  {
    id: "title",
    header: "Item",
    cell: (row) => row.title,
    sortValue: (row) => row.title,
    identifier: true,
  },
  {
    id: "state",
    header: "State",
    // The engine's words for the state; a state without them shows as sent.
    cell: (row) => row.stateLabel ?? row.state,
    width: 200,
    wrap: true,
  },
  {
    id: "since",
    header: "Waiting since",
    cell: (row) => <Timestamp value={row.since} />,
    sortValue: (row) => row.since,
    width: 190,
  },
  { id: "next", header: "Next action", cell: (row) => row.nextAction, width: 240, wrap: true },
];

/**
 * The report screen of each kind of release item: a specimen's report workspace, or a
 * released revision (partial reports and corrections due). The work queues feature maps every
 * kind; features do not share code, so this screen keeps its own two.
 */
const ITEM_SCREENS = new Map([
  ["specimen_report", workspacePath],
  ["report_revision", revisionPath],
]);

function itemPath(item: WorkQueueItem): string | null {
  return ITEM_SCREENS.get(item.itemKind)?.(item.itemId) ?? null;
}

/**
 * Release work (REQ-OPS-004, REQ-RPT-005): specimens ready for release or blocked, partial
 * reports to complete and corrections due. The queues and their order come from the engine;
 * this screen lists those owned by releasers. The chosen queue stays in the address. Other
 * people change the queues, so the time of the last read shows beside Refresh.
 */
export function ReleaseQueuePage() {
  const queues = useCommandQuery<WorkQueueSummary[]>(LIST_WORK_QUEUES);
  return (
    <Page layout="collection">
      <PageHeader title="Release queue" />
      <QueryContent
        query={queues}
        what="release queues"
        loadingRows={8}
        empty={{
          when: (all) => releaseQueues(all).length === 0,
          title: "No release queues",
          description: "Release queues are listed for people who may release reports.",
        }}
      >
        {(all) => <QueueItems queues={releaseQueues(all)} counts={queues} />}
      </QueryContent>
    </Page>
  );
}

function releaseQueues(all: readonly WorkQueueSummary[]) {
  return all.filter((queue) => queue.ownerCapability === RELEASE_REPORTS);
}

interface QueueItemsProps {
  queues: readonly WorkQueueSummary[];
  /** The read of every queue's count, refreshed with the items. */
  counts: { isFetching: boolean; isError: boolean; refetch: () => Promise<unknown> };
}

function QueueItems({ queues, counts }: QueueItemsProps) {
  // Each item opens at its own address; Previous and Next walk the items of its kind.
  const openPage = useOpenRecordPage((path) => path, "Release queue");
  const [queueParam, setQueue] = useSearchParam("queue");
  const options = queues.map((queue) => ({
    id: queue.queue,
    label: queue.label,
  }));
  const chosen = queues.find((queue) => queue.queue === queueParam) ?? queues[0];
  const queue = chosen?.queue ?? "";
  const items = useWorkQueueItems(queue, LIST_LIMIT);
  // Refresh reads the counts and the items again; "Updated" is the time of the items shown.
  const refresh = {
    dataUpdatedAt: items.dataUpdatedAt,
    isFetching: items.isFetching || counts.isFetching,
    refetch: () => Promise.all([counts.refetch(), items.restartScan()]),
  };
  const open = (row: WorkQueueItem, order: readonly string[]) => {
    const path = itemPath(row);
    const kinds = new Map(items.rows.map((item) => [item.itemId, item.itemKind]));
    if (path !== null)
      openPage(
        path,
        order.filter((id) => kinds.get(id) === row.itemKind),
      );
  };
  const total =
    chosen && chosen.count !== null && !counts.isError
      ? { count: chosen.count, capped: chosen.countCapped }
      : undefined;
  return (
    <WorkQueueList
      query={items}
      total={total}
      label={chosen?.label ?? "Queue items"}
      columns={COLUMNS}
      getRowId={(row) => row.itemId}
      onOpen={open}
      controls={
        <>
          <Select
            layout="inline"
            label="Queue"
            name="queue"
            options={options}
            value={queue}
            onChange={(next) => next && setQueue(next)}
          />
          <RefreshControl query={refresh} />
        </>
      }
    />
  );
}
