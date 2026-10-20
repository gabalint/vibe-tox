import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import { choiceOf, labelOf } from "@/lib/choices";
import { useOpenRecord } from "@/lib/openRecord";
import { useSearchParam } from "@/lib/searchParam";
import styles from "@/styles/layout.module.css";
import {
  type DeliveryQueueFilter,
  LIST_DELIVERY_OBLIGATIONS,
  LIST_LIMIT,
  type ObligationView,
  type DeliveryQueuePage as ScanPage,
} from "../contract";
import { DELIVERY_FILTERS } from "../labels";
import { useCursorPages } from "../useCursorPages";
import { ExactObligationPanel } from "./ExactObligationPanel";
import { ObligationGrid } from "./ObligationGrid";
import { useObligationActions } from "./obligationActions";

const NO_ROWS: readonly ObligationView[] = [];

type ScanPages = ReturnType<typeof useCursorPages<ScanPage>>;

/** The queue under `filter`, one bounded engine scan per page. */
function useDeliveryPages(filter: DeliveryQueueFilter) {
  // Each page is one bounded engine scan; `nextAfter` names where the next one continues, and
  // a scan may stop early with fewer rows than asked, even none.
  const pages = useCursorPages<ScanPage>(LIST_DELIVERY_OBLIGATIONS, {
    key: { filter },
    pageArgs: (after) => ({ input: { filter, limit: LIST_LIMIT, after } }),
    nextCursor: (page) => page.nextAfter ?? undefined,
  });
  const rows = useMemo(
    () => pages.data?.pages.flatMap((page) => page.items) ?? NO_ROWS,
    [pages.data],
  );
  return { pages, rows };
}

/**
 * Delivery work (REQ-RPT-003, REQ-RPT-011, REQ-OPS-004), a workspace: obligations awaiting
 * delivery, failed or of unknown outcome, blocked by an unverified or retired destination, or
 * paused for impact review. Withdrawal notices and corrections come first, in the engine's
 * order. A row opens in the side panel (`?open=`), with the actions the engine allows; every
 * status names its channel and evidence. The filter stays in the address. The engine scans the
 * queue in bounded pages, so loading more continues where the last page stopped; other people
 * record deliveries, so the time of the last read shows beside Refresh.
 *
 * An attempt usually moves the obligation out of the filter shown. Its panel then stays open
 * with the obligation as the engine returned it: the person sees the status and evidence they
 * recorded, and a failed attempt's next step (redirect, retry) stays at hand.
 */
export function DeliveryQueuePage() {
  const [filterParam, setFilter] = useSearchParam("filter");
  const filter = choiceOf(DELIVERY_FILTERS, filterParam) ?? "awaiting";
  const { pages, rows } = useDeliveryPages(filter);
  const record = useOpenRecord();
  const [recorded, setRecorded] = useState<ObligationView | null>(null);
  const actions = useObligationActions({ onRecorded: setRecorded });
  return (
    <Page layout="collection">
      <PageHeader
        title="Delivery"
        description="Staff-operated delivery of released reports and withdrawal notices."
      />
      <DetailLayout
        panel={
          <ExactObligationPanel
            filter={filter}
            rows={rows}
            recorded={recorded}
            record={record}
            actions={actions.items}
          />
        }
      >
        <Toolbar
          variant="collection"
          count={
            pages.data && (
              <ListCount
                shown={rows.length}
                noun="obligations"
                one="obligation"
                hasMore={pages.hasNextPage}
              />
            )
          }
        >
          <Select
            layout="inline"
            label="Show"
            name="filter"
            options={DELIVERY_FILTERS}
            value={filter}
            onChange={(next) => setFilter(choiceOf(DELIVERY_FILTERS, next) ?? "awaiting")}
          />
          <RefreshControl query={pages} />
        </Toolbar>
        {pages.hasNextPage && (
          <p className={styles.hint}>
            {rows.length > 0
              ? "The scan has more obligations to check; sorting applies to loaded obligations."
              : "None of the obligations checked so far match. Load more to continue the search."}
          </p>
        )}
        <QueryContent query={pages} what="delivery obligations" loadingRows={8}>
          {() => (
            <>
              {rows.length > 0 ? (
                <ObligationGrid
                  obligations={rows}
                  rowActions={actions.rowActions}
                  withReport
                  openId={record.openId}
                  onOrderChange={record.followOrder}
                  onOpen={(row, order) => record.open(row.obligationId, order)}
                  fill
                />
              ) : (
                !pages.hasNextPage && (
                  <NothingToDeliver filter={filter} onShowAll={() => setFilter("all")} />
                )
              )}
              <MorePages pages={pages} />
            </>
          )}
        </QueryContent>
      </DetailLayout>
      {actions.dialog}
    </Page>
  );
}

/** "Load more obligations" while the engine's scan has not reached the end. */
function MorePages({ pages }: { pages: ScanPages }) {
  return (
    <>
      {pages.isFetchNextPageError && (
        <ErrorState title="More obligations could not be loaded" error={pages.error} />
      )}
      {pages.hasNextPage && (
        <Toolbar>
          <Button isPending={pages.isFetchingNextPage} onPress={() => void pages.fetchNextPage()}>
            Load more obligations
          </Button>
        </Toolbar>
      )}
    </>
  );
}

interface NothingToDeliverProps {
  filter: DeliveryQueueFilter;
  onShowAll: () => void;
}

/** "Nothing at all" and "nothing matches this filter" differ: the latter offers every open one. */
function NothingToDeliver({ filter, onShowAll }: NothingToDeliverProps) {
  if (filter === "all") {
    return <EmptyState title="Nothing to deliver" description="No delivery obligation is open." />;
  }
  return (
    <EmptyState
      title={`Nothing in “${labelOf(DELIVERY_FILTERS, filter)}”`}
      description="Other open obligations may need attention."
      action={<Button onPress={onShowAll}>Show all open</Button>}
    />
  );
}
