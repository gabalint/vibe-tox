import { useMemo } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { HealthEventView } from "@/generated/HealthEventView";
import type { HostHealthView } from "@/generated/HostHealthView";
import { useCommandPages, useCommandQuery } from "@/lib/commands";
import { useSearchParam } from "@/lib/searchParam";
import { GET_HOST_HEALTH, LIST_HEALTH_HISTORY, LIST_LIMIT } from "../contract";
import { isCheckCode } from "../engineCodes";
import { HealthBadge, NONE } from "./healthLabels";

const ALL = "all";

function eventId(event: HealthEventView): string {
  return event.eventId;
}

function historyColumns(labels: ReadonlyMap<string, string>): DataGridColumn<HealthEventView>[] {
  const label = (event: HealthEventView) => labels.get(event.check) ?? event.check;
  return [
    { id: "check", header: "Check", cell: label, sortValue: label },
    {
      id: "state",
      header: "State",
      cell: (event) => <HealthBadge state={event.state} />,
      sortValue: (event) => event.state,
      width: 130,
    },
    { id: "detail", header: "Detail code", cell: (event) => event.detailCode ?? NONE },
    {
      id: "observed",
      header: "Observed",
      cell: (event) => <Timestamp value={event.observedAt} />,
      sortValue: (event) => event.observedAt,
      width: 190,
    },
  ];
}

/**
 * Each change of a check's state, newest first (REQ-OPS-008): the maintenance record. The check
 * filter stays in the address; its options are the checks the engine reports now.
 */
export function CheckHistory() {
  const [param, setParam] = useSearchParam("check");
  // A check code edited into the address that the engine does not know shows all checks.
  const check = isCheckCode(param) ? param : null;
  const health = useCommandQuery<HostHealthView>(GET_HOST_HEALTH);
  const history = useCommandPages<HealthEventView, string>(LIST_HEALTH_HISTORY, {
    key: { check },
    pageArgs: (beforeEventId) => ({ input: { check, limit: LIST_LIMIT, beforeEventId } }),
    nextCursor: (page) => (page.length > LIST_LIMIT ? page[LIST_LIMIT - 1]?.eventId : undefined),
  });
  const rows = history.data?.pages.flatMap((page) => page.slice(0, LIST_LIMIT)) ?? [];
  const hasMore = history.hasNextPage;
  const checks = health.data?.checks;
  const labels = useMemo(
    () => new Map((checks ?? []).map((each) => [each.check, each.label])),
    [checks],
  );
  const columns = useMemo(() => historyColumns(labels), [labels]);
  const options = [
    { id: ALL, label: "All checks" },
    ...[...labels].map(([id, label]) => ({ id, label })),
  ];
  if (check && !labels.has(check)) options.push({ id: check, label: check });
  return (
    <>
      <Toolbar
        variant="collection"
        count={
          history.isSuccess && (
            <ListCount
              shown={rows.length}
              noun="state changes"
              one="state change"
              hasMore={hasMore}
            />
          )
        }
      >
        <RefreshControl query={history} />
        <Select
          label="Check"
          layout="inline"
          name="check"
          options={options}
          value={check ?? ALL}
          onChange={(next) => setParam(next === ALL ? null : next)}
        />
        {check && <Button onPress={() => setParam(null)}>Clear filters</Button>}
      </Toolbar>
      <QueryContent
        query={history}
        what="health history"
        loadingRows={4}
        empty={{
          when: (data) => data.pages.flat().length === 0,
          title: "No health history",
          description: "No state change has been recorded for this selection.",
        }}
      >
        {() => (
          <>
            <DataGrid
              label="Health history"
              columns={columns}
              rows={rows}
              getRowId={eventId}
              fill
            />
          </>
        )}
      </QueryContent>
      {hasMore && (
        <Button isPending={history.isFetchingNextPage} onPress={() => void history.fetchNextPage()}>
          Load older state changes
        </Button>
      )}
    </>
  );
}
