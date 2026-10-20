import { Link, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import type { BillingReviewRecord } from "@/generated/BillingReviewRecord";
import type { BillingReviewStatus } from "@/generated/BillingReviewStatus";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { REVIEW_ELEMENTS } from "./labels";

const COLUMNS: DataGridColumn<BillingReviewRecord>[] = [
  {
    id: "element",
    header: "Financial follow-up",
    cell: (row) => (
      <Link to={`/billing/orders/${row.orderId}`}>
        {row.orderReference ? `Order ${row.orderReference}` : "Open order billing"} ·{" "}
        {labelOf(REVIEW_ELEMENTS, row.element)}
      </Link>
    ),
    identifier: true,
  },
  { id: "owner", header: "Owner", cell: (row) => row.owner ?? "Unassigned" },
  {
    id: "openedAt",
    header: "Opened",
    cell: (row) => <Timestamp value={row.openedAt} />,
    width: 190,
  },
  {
    id: "status",
    header: "Status",
    cell: (row) => (row.status === "open" ? "Open" : "Resolved"),
    width: 100,
  },
];

export function BillingReviewPage() {
  const [params, setParams] = useSearchParams();
  const status: BillingReviewStatus = params.get("status") === "resolved" ? "resolved" : "open";
  const afterItemId = params.get("after");
  const query = useCommandQuery<BillingReviewRecord[]>("list_billing_review_items", {
    input: { status, limit: 50, afterItemId },
  });
  const changeStatus = (next: BillingReviewStatus) => {
    setParams({ status: next });
  };
  return (
    <Page>
      <PageHeader
        title="Billing follow-up"
        description="Financial documentation to review, oldest first."
        actions={
          <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
            Refresh follow-up
          </Button>
        }
      />
      <div>
        <Button
          variant={status === "open" ? "primary" : "secondary"}
          onPress={() => changeStatus("open")}
        >
          Open follow-up
        </Button>{" "}
        <Button
          variant={status === "resolved" ? "primary" : "secondary"}
          onPress={() => changeStatus("resolved")}
        >
          Resolved follow-up
        </Button>
      </div>
      <QueryContent
        query={query}
        what="billing follow-up"
        empty={{
          when: (rows) => !rows.length,
          title: `No ${status} billing follow-up on this page`,
          description: "Billing follow-up is opened from an order's billing section.",
        }}
      >
        {(rows) => (
          <>
            <DataGrid
              label="Billing follow-up"
              rows={rows}
              columns={COLUMNS}
              getRowId={(row) => row.id}
            />
          </>
        )}
      </QueryContent>
      <div>
        <Button
          isDisabled={afterItemId === null || query.isFetching}
          onPress={() => setParams({ status })}
        >
          First page
        </Button>{" "}
        <Button
          isDisabled={query.data?.length !== 50 || query.isFetching}
          onPress={() => {
            const last = query.data?.at(-1);
            if (last) setParams({ status, after: last.id });
          }}
        >
          Next page
        </Button>
      </div>
    </Page>
  );
}
