import { useMemo } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { FindField } from "@/components/FindField";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { BillingHandoffSummary } from "@/generated/BillingHandoffSummary";
import { optionalText } from "@/lib/choices";
import { useCommandPages } from "@/lib/commands";
import { useFindParam } from "@/lib/findParam";
import { useOpenRecordPage } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import { LIST_BILLING_HANDOFFS, LIST_LIMIT } from "../contract";
import { itemStatus } from "../labels";
import { handoffPath } from "../paths";

const LIST_LABEL = "Billing handoffs";

/** The status that needs attention first: an item awaiting an outcome, then a rejected one. */
function handoffStatus(row: BillingHandoffSummary) {
  if (row.outstandingCount > 0) return itemStatus("outstanding");
  if (row.rejectedCount > 0) return itemStatus("rejected");
  return itemStatus("acknowledged");
}

const accessions = (row: BillingHandoffSummary) => row.accessionNumbers.join(", ");
const dates = (row: BillingHandoffSummary) =>
  row.datesOfService.map((date) => date ?? "Unknown").join(", ");

const COLUMNS: readonly DataGridColumn<BillingHandoffSummary>[] = [
  {
    id: "number",
    header: "Handoff",
    cell: (row) => row.handoffNo,
    sortValue: (row) => row.handoffNo,
    align: "end",
    width: 110,
    identifier: true,
  },
  {
    id: "created",
    header: "Created",
    cell: (row) => <Timestamp value={row.createdAt} />,
    sortValue: (row) => row.createdAt,
    width: 190,
  },
  { id: "accessions", header: "Accessions", cell: accessions, sortValue: accessions },
  { id: "dos", header: "Date of service", cell: dates, sortValue: dates, width: 150 },
  {
    id: "items",
    header: "Items",
    cell: (row) => row.itemCount,
    sortValue: (row) => row.itemCount,
    align: "end",
    width: 90,
  },
  {
    id: "status",
    header: "Status",
    cell: (row) => {
      const status = handoffStatus(row);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    sortValue: (row) => handoffStatus(row).label,
    width: 180,
  },
];

/**
 * Every billing handoff (REQ-BIL-002), newest first, including those whose items are all
 * acknowledged and so no longer in Billing work. The engine finds a handoff by its number, or
 * by the start of an accession number or a date of service; the find text stays in `?q=`. A
 * row opens the handoff, where its file is exported again and each item's outcome history is
 * shown. The list carries no patient details.
 */
export function HandoffsPage() {
  const openHandoff = useOpenRecordPage(handoffPath, LIST_LABEL);
  const [, setParams] = useSearchParams();
  const [search, setSearch] = useFindParam();
  const searched = optionalText(search);
  const handoffs = useCommandPages<BillingHandoffSummary>(LIST_BILLING_HANDOFFS, {
    key: { search: searched },
    pageArgs: (before) => ({
      input: { limit: LIST_LIMIT, search: searched, beforeHandoffNo: before ?? null },
    }),
    // The engine marks every row of a page that has older handoffs after it.
    nextCursor: (rows) => (rows[0]?.hasMore ? rows.at(-1)?.handoffNo : undefined),
  });
  const rows = useMemo(
    () =>
      Array.from(
        new Map((handoffs.data?.pages.flat() ?? []).map((row) => [row.handoffId, row])).values(),
      ),
    [handoffs.data],
  );
  return (
    <Page layout="collection">
      <PageHeader
        title={LIST_LABEL}
        description="Every handoff file given to the billing partner, newest first, with where its items stand."
      />
      <Toolbar
        variant="collection"
        count={
          handoffs.isSuccess && (
            <ListCount
              shown={rows.length}
              noun="handoffs"
              one="handoff"
              hasMore={handoffs.hasNextPage}
            />
          )
        }
      >
        <FindField
          label="Find by handoff number, accession or date of service"
          placeholder="Handoff number, accession or YYYY-MM-DD"
          value={search}
          onChange={setSearch}
        />
        <RefreshControl query={handoffs} />
      </Toolbar>
      {handoffs.hasNextPage && (
        <p className={styles.hint}>
          Newest first; sorting applies to loaded handoffs. Load older handoffs to continue.
        </p>
      )}
      <QueryContent
        query={handoffs}
        what="billing handoffs"
        loadingRows={8}
        empty={{
          when: () => rows.length === 0,
          title: searched ? "No handoff matches" : "No handoffs",
          description: searched
            ? "No handoff has this number, or an accession or date of service starting with it."
            : "Handoffs are created from Billing work, under Ready for approval or handoff.",
          action: searched && <Button onPress={() => setParams({})}>Show all handoffs</Button>,
        }}
      >
        {() => (
          <DataGrid
            label={LIST_LABEL}
            columns={COLUMNS}
            rows={rows}
            getRowId={(row) => row.handoffId}
            onRowAction={(row, order) => openHandoff(row.handoffId, order)}
            fill
          />
        )}
      </QueryContent>
      {handoffs.hasNextPage && (
        <Button
          isPending={handoffs.isFetchingNextPage}
          onPress={() => void handoffs.fetchNextPage()}
        >
          Load older handoffs
        </Button>
      )}
    </Page>
  );
}
