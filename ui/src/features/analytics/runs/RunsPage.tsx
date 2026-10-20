import { useMemo } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { RunState } from "@/generated/RunState";
import type { RunSummary } from "@/generated/RunSummary";
import { type Choice, choiceOf } from "@/lib/choices";
import { useCommandPages, useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import { LIST_INSTRUMENT_READINESS, LIST_RUNS } from "../contract";
import { runStatus } from "../labels";
import { RUNS_LIST, runPath } from "../paths";

const RUN_FILTERS: readonly Choice<RunState | "all">[] = [
  { id: "open", label: "Open runs" },
  { id: "closed", label: "Closed runs" },
  { id: "all", label: "All runs" },
];

/** The creation time identifies the run; a click or Enter on the row opens it. */
function runColumns(instrumentName: (id: string) => string): DataGridColumn<RunSummary>[] {
  return [
    {
      id: "created",
      header: "Created",
      identifier: true,
      cell: (run) => <Timestamp value={run.createdAt} />,
      sortValue: (run) => run.createdAt,
      width: 200,
    },
    {
      id: "instrument",
      header: "Instrument",
      cell: (run) => instrumentName(run.instrumentId),
      sortValue: (run) => instrumentName(run.instrumentId),
    },
    {
      id: "positions",
      header: "Positions",
      cell: (run) => run.positionCount,
      sortValue: (run) => run.positionCount,
      align: "end",
      width: 120,
    },
    {
      id: "state",
      header: "State",
      cell: (run) => {
        const status = runStatus(run.state);
        return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
      },
      width: 120,
    },
  ];
}

/**
 * Runs newest first, for analysts (REQ-ANA-001). The filter stays in the address. Other
 * analysts start and close runs, so the list says when it was read.
 */
export function RunsPage() {
  const navigate = useNavigate();
  const open = useOpenRecordPage(runPath, RUNS_LIST.label);
  const [stateParam, setState] = useSearchParam("state");
  const filter = choiceOf(RUN_FILTERS, stateParam) ?? "open";
  const runs = useCommandPages<RunSummary, string>(LIST_RUNS, {
    key: { filter },
    pageArgs: (afterRunId) => ({
      input: { state: filter === "all" ? undefined : filter, afterRunId, limit: 200 },
    }),
    nextCursor: (page) => (page.length === 200 ? page.at(-1)?.runId : undefined),
  });
  const rows = useMemo(() => runs.data?.pages.flat() ?? [], [runs.data]);
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const columns = useMemo(
    () =>
      runColumns(
        (id) => instruments.data?.find((instrument) => instrument.instrumentId === id)?.name ?? id,
      ),
    [instruments.data],
  );
  return (
    <Page layout="collection">
      <PageHeader
        title={RUNS_LIST.label}
        actions={
          <Button variant="primary" onPress={() => navigate("/bench/worklist")}>
            Go to worklist
          </Button>
        }
      />
      <Toolbar
        variant="collection"
        count={runs.data && <ListCount shown={rows.length} noun="runs" one="run" />}
      >
        <Select
          layout="inline"
          label="Show"
          name="state"
          options={RUN_FILTERS}
          value={filter}
          onChange={(next) => setState(choiceOf(RUN_FILTERS, next) ?? "open")}
        />
        <RefreshControl query={runs} />
      </Toolbar>
      <QueryContent
        query={runs}
        what="runs"
        loadingRows={8}
        empty={{
          when: (data) => data.pages.flat().length === 0,
          title: "No runs",
          description: "No runs match this filter.",
        }}
      >
        {() => (
          <DataGrid
            label="Runs"
            columns={columns}
            rows={rows}
            getRowId={(run) => run.runId}
            onRowAction={(run, order) => open(run.runId, order)}
            fill
          />
        )}
      </QueryContent>
      {runs.hasNextPage && (
        <Button isPending={runs.isFetchingNextPage} onPress={() => void runs.fetchNextPage()}>
          Load older runs
        </Button>
      )}
    </Page>
  );
}
