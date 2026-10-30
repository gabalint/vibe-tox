import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { useCommandPages, useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import {
  LIST_IMPORT_ATTEMPTS,
  LIST_INSTRUMENT_READINESS,
  LIST_LIMIT,
  PERFORM_ANALYSIS,
} from "../contract";
import { IMPORTS_LIST, importAttemptPath } from "../paths";
import { ImportOutcomeBadge } from "./ImportOutcomeBadge";
import { LoadResultFileDialog } from "./LoadResultFileDialog";

/** How the attempt's data arrived: a result file, by hand or from the folder, or a message. */
function originText(row: ImportAttemptSummary): string {
  if (row.origin === "upload") return "File loaded by hand";
  if (row.origin === "folder") return "File from the watched folder";
  return "Analyzer message";
}

/** The filter's choice for every instrument; the URL then has no `instrument`. */
const ALL_INSTRUMENTS = "all";

/** The processing time identifies the attempt; a click or Enter on the row opens it. */
function attemptColumns(
  instrumentName: (id: string) => string,
): DataGridColumn<ImportAttemptSummary>[] {
  return [
    {
      id: "processed",
      header: "Processed",
      identifier: true,
      cell: (row) => <Timestamp value={row.processedAt} />,
      sortValue: (row) => row.processedAt,
      width: 200,
    },
    {
      id: "instrument",
      header: "Instrument",
      cell: (row) => instrumentName(row.instrumentId),
      sortValue: (row) => instrumentName(row.instrumentId),
    },
    { id: "origin", header: "Received as", cell: originText, width: 220 },
    {
      id: "attempt",
      header: "Attempt",
      cell: (row) => row.attemptNo,
      align: "end",
      width: 100,
    },
    {
      id: "outcome",
      header: "Outcome",
      cell: (row) => <ImportOutcomeBadge outcome={row.outcome} />,
      width: 260,
    },
    { id: "rows", header: "Rows", cell: (row) => row.rowsTotal, align: "end", width: 90 },
    {
      id: "quarantined",
      header: "Quarantined",
      cell: (row) => row.rowsQuarantined,
      sortValue: (row) => row.rowsQuarantined,
      align: "end",
      width: 130,
    },
  ];
}

/**
 * Each attempt to import a received analyzer message (REQ-ANA-003), newest first, with its
 * counts only. Opening one shows its quarantined rows. New messages arrive while the screen is
 * open, so the list says when it was read. `?instrument=` shows one instrument's imports, as
 * its panel under Laboratory setup links them.
 */
export function ImportAttemptsPage() {
  const [instrumentId, setInstrumentId] = useSearchParam("instrument");
  const attempts = useCommandPages<ImportAttemptSummary, string>(LIST_IMPORT_ATTEMPTS, {
    key: { directory: true, instrumentId },
    pageArgs: (afterImportAttemptId) => ({
      input: { instrumentId, limit: LIST_LIMIT, before: null, afterImportAttemptId },
    }),
    nextCursor: (page) =>
      page.length > LIST_LIMIT ? page[LIST_LIMIT - 1]?.importAttemptId : undefined,
  });
  const rows = attempts.data?.pages.flatMap((page) => page.slice(0, LIST_LIMIT)) ?? [];
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const columns = useMemo(
    () =>
      attemptColumns(
        (id) => instruments.data?.find((instrument) => instrument.instrumentId === id)?.name ?? id,
      ),
    [instruments.data],
  );
  return (
    <Page layout="collection">
      <PageHeader title={IMPORTS_LIST.label} actions={<LoadResultFile />} />
      <Toolbar
        variant="collection"
        count={
          attempts.data && (
            <ListCount
              shown={rows.length}
              noun="imports"
              one="import"
              hasMore={attempts.hasNextPage}
            />
          )
        }
      >
        <Select
          layout="inline"
          label="Instrument"
          name="instrument"
          options={[
            { id: ALL_INSTRUMENTS, label: "All instruments" },
            ...(instruments.data ?? []).map((instrument) => ({
              id: instrument.instrumentId,
              label: instrument.name,
            })),
          ]}
          value={instrumentId ?? ALL_INSTRUMENTS}
          onChange={(next) => setInstrumentId(next === ALL_INSTRUMENTS ? null : next)}
        />
        <RefreshControl query={attempts} />
      </Toolbar>
      <QueryContent
        query={attempts}
        what="imports"
        loadingRows={8}
        empty={{
          when: (data) => data.pages.flat().length === 0,
          title: "No imports",
          description:
            instrumentId === null
              ? "No analyzer message has been processed yet."
              : "No message from this instrument has been processed yet.",
        }}
      >
        {() => <AttemptList answer={rows} columns={columns} />}
      </QueryContent>
      {attempts.hasNextPage && (
        <Button
          isPending={attempts.isFetchingNextPage}
          onPress={() => void attempts.fetchNextPage()}
        >
          Load older imports
        </Button>
      )}
    </Page>
  );
}

interface AttemptListProps {
  answer: readonly ImportAttemptSummary[];
  columns: readonly DataGridColumn<ImportAttemptSummary>[];
}

function AttemptList({ answer, columns }: AttemptListProps) {
  const open = useOpenRecordPage(importAttemptPath, IMPORTS_LIST.label);
  const rows = answer;
  return (
    <DataGrid
      label="Imports"
      columns={columns}
      rows={rows}
      getRowId={(row) => row.importAttemptId}
      onRowAction={(row, order) => open(row.importAttemptId, order)}
      fill
    />
  );
}

/** Loading a results file by hand is analytical work (`perform_analysis`, F16). */
function LoadResultFile() {
  const canLoad = useHasCapability(PERFORM_ANALYSIS);
  const [isLoading, setLoading] = useState(false);
  if (!canLoad) return null;
  return (
    <>
      <Button variant="primary" onPress={() => setLoading(true)}>
        Load results file
      </Button>
      {isLoading && <LoadResultFileDialog onClose={() => setLoading(false)} />}
    </>
  );
}
