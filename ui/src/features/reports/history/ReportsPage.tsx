import { useMemo } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ErrorState } from "@/components/ErrorState";
import { FindField } from "@/components/FindField";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import { choiceOf, optionalText } from "@/lib/choices";
import { useFindParam } from "@/lib/findParam";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import styles from "@/styles/layout.module.css";
import { LIST_LIMIT, LIST_REPORTS, type ReportSummary } from "../contract";
import { REPORT_FILTERS } from "../labels";
import { reportPath } from "../paths";
import { useCursorPages } from "../useCursorPages";
import { statusColumns } from "./columns";

const COLUMNS: readonly DataGridColumn<ReportSummary>[] = [
  {
    id: "number",
    header: "Report",
    cell: (row) => row.reportNumber,
    sortValue: (row) => row.reportNumber,
    identifier: true,
  },
  {
    id: "accession",
    header: "Accession",
    cell: (row) => row.accessionNumber,
    sortValue: (row) => row.accessionNumber,
    width: 160,
  },
  {
    id: "revision",
    header: "Latest revision",
    cell: (row) => row.latestRevisionNo,
    sortValue: (row) => row.latestRevisionNo,
    align: "end",
    width: 140,
  },
  ...statusColumns<ReportSummary>(),
  {
    id: "released",
    header: "Released",
    cell: (row) => <Timestamp value={row.releasedAt} />,
    sortValue: (row) => row.releasedAt,
    width: 190,
  },
];

/**
 * Released reports (REQ-RPT-002), latest release first: partial, withdrawn or all, searchable
 * by the start of the report number. The filter and the find text (`?q=`) stay in the address. The list
 * carries no patient details. Older releases load on request, a page at a time, from where
 * the engine's last page ended.
 */
export function ReportsPage() {
  const openReport = useOpenRecordPage(reportPath, "Reports");
  const [, setParams] = useSearchParams();
  const [filterParam, setFilter] = useSearchParam("filter");
  const filter = choiceOf(REPORT_FILTERS, filterParam) ?? "all";
  // The engine finds reports whose number starts with the text, kept in `?q=`.
  const [search, setSearch] = useFindParam();
  const searched = optionalText(search) ?? undefined;
  const reports = useCursorPages<ReportSummary[]>(LIST_REPORTS, {
    key: { filter, search: searched },
    pageArgs: (cursor) => {
      const [beforeReleasedAt, beforeReportId] = cursor?.split("|") ?? [];
      return {
        input: { filter, search: searched, limit: LIST_LIMIT, beforeReleasedAt, beforeReportId },
      };
    },
    // The engine marks every row of a page that has older reports after it.
    nextCursor: (page) => {
      const last = page.at(-1);
      return page[0]?.hasMore && last ? `${last.releasedAt}|${last.reportId}` : undefined;
    },
  });
  const filtered = filter !== "all" || search !== "";
  const shown = reports.data?.pages.reduce((count, page) => count + page.length, 0);
  return (
    <Page layout="collection">
      <PageHeader title="Reports" />
      <RefreshControl query={reports} />
      <Toolbar
        variant="collection"
        count={
          shown !== undefined && (
            <ListCount shown={shown} noun="reports" one="report" hasMore={reports.hasNextPage} />
          )
        }
      >
        <FindField
          label="Find by report number"
          placeholder="Report number"
          value={search}
          onChange={setSearch}
        />
        <Select
          layout="inline"
          label="Show"
          name="filter"
          options={REPORT_FILTERS}
          value={filter}
          onChange={(next) => setFilter(choiceOf(REPORT_FILTERS, next) ?? "all")}
        />
      </Toolbar>
      {reports.hasNextPage && (
        <p className={styles.hint}>
          Latest release first; sorting applies to loaded reports. Load older reports to continue.
        </p>
      )}
      <QueryContent
        query={reports}
        what="reports"
        loadingRows={8}
        empty={{
          when: (data) => data.pages.every((page) => page.length === 0),
          // "Nothing released yet" differs from "the filter or search excludes everything".
          title: filtered ? "No report matches" : "No reports",
          description: filtered
            ? "No released report matches this filter and search."
            : "No report has been released yet.",
          action: filtered && <Button onPress={() => setParams({})}>Show all reports</Button>,
        }}
      >
        {() => (
          <ReportList pages={reports} onOpen={(row, order) => openReport(row.reportId, order)} />
        )}
      </QueryContent>
    </Page>
  );
}

interface ReportListProps {
  pages: ReturnType<typeof useCursorPages<ReportSummary[]>>;
  onOpen: (row: ReportSummary, order: readonly string[]) => void;
}

/** The loaded pages as one list, then "Load older reports" while the engine has more. */
function ReportList({ pages, onOpen }: ReportListProps) {
  const rows = useMemo(
    () =>
      Array.from(
        new Map((pages.data?.pages.flat() ?? []).map((row) => [row.reportId, row])).values(),
      ),
    [pages.data],
  );
  return (
    <>
      <DataGrid
        label="Reports"
        columns={COLUMNS}
        rows={rows}
        getRowId={(row) => row.reportId}
        onRowAction={onOpen}
        fill
      />
      {pages.isFetchNextPageError && (
        <ErrorState title="Older reports could not be loaded" error={pages.error} />
      )}
      {pages.hasNextPage && (
        <Toolbar>
          <Button isPending={pages.isFetchingNextPage} onPress={() => void pages.fetchNextPage()}>
            Load older reports
          </Button>
        </Toolbar>
      )}
    </>
  );
}
