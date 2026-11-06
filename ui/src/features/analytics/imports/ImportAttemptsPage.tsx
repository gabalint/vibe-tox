import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { useCommandPages, useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import {
  LIST_IMPORT_ATTEMPTS,
  LIST_INSTRUMENT_READINESS,
  LIST_LIMIT,
  PERFORM_ANALYSIS,
} from "../contract";
import { IMPORTS_LIST, importAttemptPath } from "../paths";
import { attemptColumns } from "./attemptColumns";
import { ImportFilters } from "./ImportFilters";
import { type ImportFilter, isFiltered, useImportFilter, useImportListInput } from "./importFilter";
import { LoadResultFileDialog } from "./LoadResultFileDialog";

/**
 * Each attempt to import a received analyzer message (REQ-ANA-003), newest first, with its
 * counts only. Opening one shows its quarantined rows. The instrument, outcome, received days
 * and a search by sample label, accession, container label or file name stay in the address
 * and are applied by the engine, before the list is paged; the instrument panel under
 * Laboratory setup links one instrument's imports with `?instrument=`. New messages arrive
 * while the screen is open, so the list says when it was read.
 */
export function ImportAttemptsPage() {
  const [filter, applyFilter] = useImportFilter();
  const input = useImportListInput(filter);
  const attempts = useCommandPages<ImportAttemptSummary, string>(LIST_IMPORT_ATTEMPTS, {
    key: { directory: true, ...input },
    pageArgs: (afterImportAttemptId) => ({
      input: { ...input, limit: LIST_LIMIT, before: null, afterImportAttemptId },
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
      <ImportFilters
        filter={filter}
        instruments={instruments.data ?? []}
        onApply={applyFilter}
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
        actions={<RefreshControl query={attempts} />}
      />
      <QueryContent
        query={attempts}
        what="imports"
        loadingRows={8}
        empty={{
          when: (data) => data.pages.flat().length === 0,
          title: "No imports",
          description: emptyText(filter),
          action: isFiltered(filter) && (
            <Button onPress={() => applyFilter({})}>Clear filters</Button>
          ),
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

function emptyText(filter: ImportFilter): string {
  if (!isFiltered(filter)) return "No analyzer message has been processed yet.";
  const { instrumentId, ...others } = filter;
  return instrumentId && !isFiltered(others)
    ? "No message from this instrument has been processed yet."
    : "No import matches these filters.";
}

interface AttemptListProps {
  answer: readonly ImportAttemptSummary[];
  columns: readonly DataGridColumn<ImportAttemptSummary>[];
}

function AttemptList({ answer, columns }: AttemptListProps) {
  const open = useOpenRecordPage(importAttemptPath, IMPORTS_LIST.label);
  return (
    <DataGrid
      label="Imports"
      columns={columns}
      rows={answer}
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
