import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { EmptyState } from "@/components/EmptyState";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import type { OrderFilter } from "@/generated/OrderFilter";
import type { OrderSummary } from "@/generated/OrderSummary";
import { choiceOf } from "@/lib/choices";
import { useCommandPages } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import { newOrderPath } from "../subjects";
import { LIST_ORDERS } from "./contract";
import { ORDER_FILTERS } from "./labels";
import { ORDER_COLUMNS, orderPath } from "./orderColumns";

/** Each explicit page holds at most 200 orders; all earlier orders remain reachable. */
const ORDER_LIMIT = 200;

/**
 * Orders newest first, filtered by what they need (REQ-ORD-001), each named by patient,
 * order number and client. The filter, and the patient when opened from a patient record,
 * stay in the address. Each listing is an audited access, so it refreshes on request.
 */
export function OrdersPage() {
  const navigate = useNavigate();
  const [filterParam, setFilter] = useSearchParam("filter");
  const [subjectId, setSubjectId] = useSearchParam("subjectId");
  const filter = choiceOf(ORDER_FILTERS, filterParam) ?? "all";
  const orders = useCommandPages<OrderSummary, string>(LIST_ORDERS, {
    key: { filter, subjectId },
    pageArgs: (beforeOrderId) => ({
      input: {
        filter,
        subjectId,
        limit: ORDER_LIMIT,
        beforeReceivedAt: null,
        beforeOrderId: beforeOrderId ?? null,
      },
    }),
    nextCursor: (page) => (page.length === ORDER_LIMIT ? page.at(-1)?.orderId : undefined),
  });
  const rows = orders.data?.pages.flat() ?? [];
  const hasMore = orders.hasNextPage;
  return (
    <Page layout="collection">
      <PageHeader
        title="Orders"
        description={subjectId ? "Orders for one patient, newest first." : undefined}
        actions={
          <Button
            variant="primary"
            onPress={() => navigate(subjectId ? newOrderPath(subjectId) : "/orders/new")}
          >
            New order
          </Button>
        }
      />
      {subjectId && <IdentityBanner subjectId={subjectId} />}
      <Toolbar
        variant="collection"
        count={
          orders.isSuccess && (
            <ListCount shown={rows.length} noun="orders" one="order" hasMore={hasMore} />
          )
        }
      >
        <Select
          label="Show orders"
          layout="inline"
          name="filter"
          options={ORDER_FILTERS}
          value={filter}
          onChange={(next) => setFilter(choiceOf(ORDER_FILTERS, next) ?? "all")}
        />
        {filter !== "all" && <Button onPress={() => setFilter("all")}>Clear filter</Button>}
        {subjectId && (
          <Button onPress={() => setSubjectId(null)}>Show orders for all patients</Button>
        )}
        <RefreshControl query={orders} />
      </Toolbar>
      <QueryContent query={orders} what="orders" loadingRows={8}>
        {() =>
          rows.length > 0 ? (
            <>
              <OrderList rows={rows} />
              {hasMore && (
                <Button
                  isPending={orders.isFetchingNextPage}
                  onPress={() => void orders.fetchNextPage()}
                >
                  Load older orders
                </Button>
              )}
            </>
          ) : (
            <NoOrders filter={filter} subjectId={subjectId} onShowAll={() => setFilter("all")} />
          )
        }
      </QueryContent>
    </Page>
  );
}

/** What an empty answer means for each filter; the list cannot tell whether others exist. */
const NONE: Record<OrderFilter, string> = {
  needs_decision: "No orders need a decision",
  on_hold: "No orders are on hold",
  accepted: "No accepted orders",
  all: "No orders yet",
};

/**
 * Nothing at all yet, or nothing in the chosen part of the worklist. The engine answers only
 * the filter asked for, so an empty filter is said as such, never as a search that matched
 * nothing.
 */
function NoOrders({
  filter,
  subjectId,
  onShowAll,
}: {
  filter: OrderFilter;
  subjectId: string | null;
  onShowAll: () => void;
}) {
  const showAll = filter !== "all" && <Button onPress={onShowAll}>Show all orders</Button>;
  const description =
    filter !== "all"
      ? "Show all orders to broaden the list."
      : subjectId
        ? "Use New order to enter an order for this patient."
        : "Use New order to choose a patient and enter their order.";
  const title = filter === "all" && subjectId ? "No orders for this patient yet" : NONE[filter];
  return <EmptyState title={title} description={description} action={showAll} />;
}

/** The orders shown; a click or Enter on a row opens the order, with Previous and Next. */
function OrderList({ rows }: { rows: readonly OrderSummary[] }) {
  const open = useOpenRecordPage(orderPath, "Orders");
  return (
    <DataGrid
      label="Orders"
      columns={ORDER_COLUMNS}
      rows={rows}
      getRowId={(order) => order.orderId}
      onRowAction={(order, ids) => open(order.orderId, ids)}
      fill
    />
  );
}
