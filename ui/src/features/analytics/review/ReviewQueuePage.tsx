import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { StatusBadge } from "@/components/StatusBadge";
import { Toolbar } from "@/components/Toolbar";
import type { ReviewQueueCursor } from "@/generated/ReviewQueueCursor";
import type { ReviewQueueEntry } from "@/generated/ReviewQueueEntry";
import { choiceOf } from "@/lib/choices";
import { QueueScanControls } from "@/lib/QueueScanControls";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import { useBoundedQueue } from "@/lib/useBoundedQueue";
import { LIST_REVIEW_QUEUE } from "../contract";
import { REVIEW_LIST, specimenResultsPath } from "../paths";
import { QUEUE_FILTERS } from "../resultLabels";

/** A count of lines needing something: a muted 0, or the count in a badge. */
function countBadge(count: number, tone: "warning" | "danger" | "info") {
  return count === 0 ? 0 : <StatusBadge tone={tone}>{String(count)}</StatusBadge>;
}

const COLUMNS: readonly DataGridColumn<ReviewQueueEntry>[] = [
  {
    id: "accession",
    header: "Accession",
    identifier: true,
    cell: (row) => row.accessionNumber,
    sortValue: (row) => row.accessionNumber,
  },
  {
    id: "patient",
    header: "Patient",
    cell: (row) => `${row.subject.familyName}, ${row.subject.givenNames ?? ""}`,
  },
  { id: "dob", header: "Date of birth", cell: (row) => row.subject.birthDate },
  {
    id: "identifiers",
    header: "Patient identifiers",
    cell: (row) =>
      row.subject.identifiers.map((i) => `${i.assigningAuthority}: ${i.value}`).join("; ") ||
      "None supplied",
  },
  { id: "type", header: "Specimen type", cell: (row) => row.specimenTypeCode, width: 140 },
  {
    id: "lines",
    header: "Results",
    cell: (row) => row.lines,
    sortValue: (row) => row.lines,
    align: "end",
    width: 110,
  },
  {
    id: "pending",
    header: "Waiting",
    cell: (row) => countBadge(row.pending, "warning"),
    sortValue: (row) => row.pending,
    align: "end",
    width: 110,
  },
  {
    id: "stale",
    header: "Review out of date",
    cell: (row) => countBadge(row.stale, "warning"),
    sortValue: (row) => row.stale,
    align: "end",
    width: 170,
  },
  {
    id: "blocked",
    header: "Blocked",
    cell: (row) => countBadge(row.blocked, "danger"),
    sortValue: (row) => row.blocked,
    align: "end",
    width: 110,
  },
];

/**
 * Specimens with results to review (REQ-RES-005), oldest first, by what they need. The
 * filter stays in the address. A row opens the specimen's results, with Previous and Next
 * through the queue. Other reviewers change the queue, so it says when it was read. The
 * entry carries patient name, birth date and supplied identifiers with its accession.
 */
export function ReviewQueuePage() {
  const [filterParam, setFilter] = useSearchParam("filter");
  const filter = choiceOf(QUEUE_FILTERS, filterParam) ?? "ready";
  const queue = useBoundedQueue<ReviewQueueEntry, ReviewQueueCursor>(LIST_REVIEW_QUEUE, {
    key: { filter },
    pageArgs: (after) => ({ input: { filter, limit: 50, after } }),
    rowId: (row) => row.specimenId,
  });
  const rows = queue.rows;
  return (
    <Page layout="collection">
      <PageHeader title={REVIEW_LIST.label} />
      <Toolbar
        variant="collection"
        count={queue.data && <ListCount shown={rows.length} noun="specimens" one="specimen" />}
      >
        <Select
          layout="inline"
          label="Show"
          name="filter"
          options={QUEUE_FILTERS}
          value={filter}
          onChange={(next) => setFilter(choiceOf(QUEUE_FILTERS, next) ?? "ready")}
        />
        <RefreshControl query={{ ...queue, refetch: queue.restartScan }} />
      </Toolbar>
      <QueryContent
        query={queue}
        what="review queue"
        loadingRows={8}
        empty={{
          when: () => rows.length === 0 && !queue.hasNextPage,
          title: "Nothing to review",
          description: "No specimen matches this filter.",
        }}
      >
        {() => <ReviewList answer={rows} />}
      </QueryContent>
      {queue.data && <QueueScanControls query={queue} review />}
    </Page>
  );
}

function ReviewList({ answer }: { answer: readonly ReviewQueueEntry[] }) {
  const open = useOpenRecordPage(specimenResultsPath, REVIEW_LIST.label);
  const rows = answer;
  return (
    <DataGrid
      label="Specimens to review"
      columns={COLUMNS}
      rows={rows}
      getRowId={(row) => row.specimenId}
      onRowAction={(row, order) => open(row.specimenId, order)}
      fill
    />
  );
}
