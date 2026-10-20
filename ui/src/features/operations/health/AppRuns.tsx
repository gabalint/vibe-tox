import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { ListCount } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Toolbar } from "@/components/Toolbar";
import type { AppRunView } from "@/generated/AppRunView";
import { allowedActionItems, allowedRowActions } from "@/lib/allowedActions";
import { useCommandPages, useCommandQuery, useHasCapability } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import {
  ACKNOWLEDGE_LISTENING_GAP,
  APP_RUNS_LIMIT,
  LIST_APP_RUNS,
  PERFORM_ANALYSIS,
} from "../contract";
import { AcknowledgeDialog } from "./AcknowledgeDialog";
import { AppRunPanel } from "./AppRunPanel";
import { RUN_COLUMNS } from "./runColumns";

const runId = (run: AppRunView) => run.runId;
const NO_RUNS: readonly AppRunView[] = [];
const ACTIONS = [{ id: ACKNOWLEDGE_LISTENING_GAP, label: "Acknowledge listening gap" }];

/**
 * App runs (REQ-OPS-002, REQ-OPS-013), newest first; a run opens in a side panel (`?open=`
 * takes its ID). While the app was closed or the computer slept, or the clock jumped ahead
 * while analyzers were connected, no analyzer results were received; someone with
 * `perform_analysis` acknowledges a run's gaps once they have checked for results that may
 * need to be sent again, from the panel or the row's menu. The engine decides which runs
 * need it.
 */
export function AppRuns() {
  const canAcknowledge = useHasCapability(PERFORM_ANALYSIS);
  const formatTime = useFormatTime();
  const record = useOpenRecord();
  const [acknowledging, setAcknowledging] = useState<AppRunView | null>(null);
  const runs = useCommandPages<AppRunView, string>(LIST_APP_RUNS, {
    key: {},
    pageArgs: (beforeRunId) => ({ input: { limit: APP_RUNS_LIMIT, beforeRunId } }),
    nextCursor: (page) => (page.length === APP_RUNS_LIMIT ? page.at(-1)?.runId : undefined),
  });
  const rows = runs.data?.pages.flat() ?? NO_RUNS;
  const selectedOutsidePage =
    Boolean(record.openId) && !rows.some((row) => row.runId === record.openId);
  const exact = useCommandQuery<AppRunView[]>(
    LIST_APP_RUNS,
    { input: { limit: 1, runId: record.openId } },
    { enabled: selectedOutsidePage },
  );
  const refresh = () => {
    void runs.refetch();
    if (selectedOutsidePage) void exact.refetch();
  };
  const panelRows = [
    ...rows,
    ...(exact.data ?? []).filter((row) => !rows.some((loaded) => loaded.runId === row.runId)),
  ];
  // Engine request: `allowedActions` on `AppRunView` would replace this condition.
  const allowed = (run: AppRunView) =>
    canAcknowledge && run.needsAcknowledgement ? [ACKNOWLEDGE_LISTENING_GAP] : [];
  const actionsOf = (run: AppRunView) =>
    allowedActionItems(ACTIONS, allowed(run), () => setAcknowledging(run));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: AppRunView) => `run started ${formatTime(row.startedAt)}`,
    allowed,
    onAction: (_, row) => setAcknowledging(row),
  });
  const panel = openRecordPanel(
    record,
    panelRows,
    runId,
    (run) => (
      <AppRunPanel
        run={run}
        actions={actionsOf(run)}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
    <QueryContent
      query={exact}
      what="selected application run"
      empty={{
        when: (data) => data.length === 0,
        title: "Application run unavailable",
        description: "The selected run was not found. Refresh the history or return to the queue.",
      }}
    >
      {() => <p>The selected application run is unavailable. Refresh the history.</p>}
    </QueryContent>,
  );
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={
            runs.isSuccess && (
              <ListCount shown={rows.length} noun="application runs" one="application run" />
            )
          }
        />
        <RefreshControl
          query={{ ...runs, isFetching: runs.isFetching || exact.isFetching, refetch: refresh }}
        />
        <p>
          Ordered by recorded start time. Clock changes may differ from the actual order of starts.
        </p>
        <QueryContent
          query={runs}
          what="app runs"
          loadingRows={4}
          empty={{
            when: (data) => data.pages.every((page) => page.length === 0),
            title: "No app runs",
            description: "The app records each start and stop here.",
          }}
        >
          {() => (
            <>
              {runs.hasNextPage && (
                <Button
                  isPending={runs.isFetchingNextPage}
                  onPress={() => void runs.fetchNextPage()}
                >
                  Load older runs
                </Button>
              )}
              <DataGrid
                label="App runs"
                columns={RUN_COLUMNS}
                rows={rows}
                getRowId={runId}
                openRowId={record.openId}
                onOrderChange={record.followOrder}
                onRowAction={(row, order) => record.open(row.runId, order)}
                rowActions={rowActions}
                fill
              />
            </>
          )}
        </QueryContent>
      </DetailLayout>
      {acknowledging && (
        <AcknowledgeDialog run={acknowledging} onClose={() => setAcknowledging(null)} />
      )}
    </>
  );
}
