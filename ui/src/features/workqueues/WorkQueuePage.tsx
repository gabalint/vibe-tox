import { useNavigate, useParams } from "react-router";
import { Button } from "@/components/Button";
import type { DataGridColumn } from "@/components/DataGrid";
import { Page, PageHeader } from "@/components/Page";
import { RefreshControl } from "@/components/RefreshControl";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useWorkQueueItems } from "@/lib/useWorkQueueItems";
import { WorkQueueList } from "@/lib/WorkQueueList";
import { Age } from "./Age";
import {
  ITEM_LIMIT,
  LIST_WORK_QUEUES,
  type WorkQueueItem,
  type WorkQueueSummary,
} from "./contract";
import { itemTarget } from "./itemPath";
import styles from "./workqueues.module.css";

/** Items of different kinds may share an identifier. */
function rowId(item: WorkQueueItem): string {
  return `${item.itemKind}:${item.itemId}`;
}

/** The item's state in the engine's words; a state without them shows as sent. */
const stateText = (item: WorkQueueItem) => item.stateLabel ?? item.state;

const COLUMNS: readonly DataGridColumn<WorkQueueItem>[] = [
  {
    id: "title",
    header: "Item",
    // An item without a screen does not open, so its title is not styled as a link.
    cell: (row) =>
      itemTarget(row) === null ? <span className={styles.noScreen}>{row.title}</span> : row.title,
    sortValue: (row) => row.title,
    identifier: true,
  },
  {
    id: "state",
    header: "State",
    cell: stateText,
    sortValue: stateText,
    width: 220,
    wrap: true,
  },
  {
    id: "age",
    header: "Waiting",
    cell: (row) => <Age since={row.since} />,
    sortValue: (row) => row.since,
    width: 320,
  },
  { id: "next", header: "Next action", cell: (row) => row.nextAction },
];

/**
 * The record IDs, in display order and once each, of the shown items of `item`'s kind: the
 * records its screen's Previous and Next walk through.
 */
function sameKindRecords(
  item: WorkQueueItem,
  rows: readonly WorkQueueItem[],
  order: readonly string[],
): string[] {
  const byId = new Map(rows.map((row) => [rowId(row), row]));
  const ids = order.flatMap((id) => {
    const row = byId.get(id);
    const target = row?.itemKind === item.itemKind ? itemTarget(row) : null;
    return target ? [target.recordId] : [];
  });
  return [...new Set(ids)];
}

/**
 * The items of one work queue (REQ-OPS-004), oldest first as the engine sorts them. An item
 * opens its exact record, where its next action is taken, with the queue in the record's
 * breadcrumb and Previous and Next through the queue's items of its kind. The queue's count
 * comes from the engine's summary, so a list cut at the limit says how many wait.
 */
export function WorkQueuePage() {
  const navigate = useNavigate();
  const queue = useParams().queue ?? "";
  const queues = useCommandQuery<WorkQueueSummary[]>(LIST_WORK_QUEUES);
  const summary = queues.data?.find((candidate) => candidate.queue === queue);
  // The code is not a title: a queue missing from the list (still loading, or not one of the
  // caller's) is named generically.
  const label = summary?.label ?? "Work queue";
  const items = useWorkQueueItems(queue, ITEM_LIMIT);
  // Items of one queue may open different screens, so each opens at its own address; the
  // sequence holds record IDs, as the record's screen knows them.
  const openPage = useOpenRecordPage((path) => path, label);
  // Refresh reads the counts and the items again; "Updated" is the time of the items shown.
  const refresh = {
    dataUpdatedAt: items.dataUpdatedAt,
    isFetching: items.isFetching || queues.isFetching,
    refetch: () => Promise.all([queues.refetch(), items.restartScan()]),
  };
  const open = (row: WorkQueueItem, order: readonly string[]) => {
    const target = itemTarget(row);
    if (target !== null) openPage(target.path, sameKindRecords(row, items.rows ?? [], order));
  };
  const total =
    summary && summary.count !== null && !queues.isError
      ? { count: summary.count, capped: summary.countCapped }
      : undefined;
  return (
    <Page layout="collection">
      <PageHeader
        title={label}
        description="Oldest first. Items without a screen show their next action only."
        actions={<Button onPress={() => navigate("/work-queues")}>All queues</Button>}
      />
      <WorkQueueList
        query={items}
        total={total}
        label={`Items in ${label}`}
        columns={COLUMNS}
        getRowId={rowId}
        onOpen={open}
        controls={<RefreshControl query={refresh} />}
      />
    </Page>
  );
}
