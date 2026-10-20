import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { ListCount, ListLimit, withinLimit } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Toolbar } from "@/components/Toolbar";
import type { ExitExportView } from "@/generated/ExitExportView";
import type { LimitInput } from "@/generated/LimitInput";
import { useHasCapability } from "@/lib/commands";
import { EXPORT, LIST_EXIT_EXPORTS, LIST_LIMIT } from "../contract";
import { isUnfinished, useJobQuery } from "../useJobQuery";
import { ExitExportDialog } from "./ExitExportDialog";
import { EXIT_EXPORT_COLUMNS } from "./exitExportColumns";

/**
 * The exit export (REQ-OPS-010, NFR-12): all of the laboratory's data as a plain folder
 * package it can take to another system. The list shows each export's state as the engine
 * reports it, newest first up to a limit, and says when older ones exist.
 */
export function ExitExportPage() {
  // One more than shown, so the list can say that more exist.
  const input: LimitInput = { limit: LIST_LIMIT + 1 };
  const query = useJobQuery<ExitExportView[]>(LIST_EXIT_EXPORTS, { input }, (rows) =>
    rows.some((row) => isUnfinished(row.state)),
  );
  const { rows, hasMore } = useMemo(() => withinLimit(query.data ?? [], LIST_LIMIT), [query.data]);
  const canExport = useHasCapability(EXPORT);
  const [exporting, setExporting] = useState(false);
  return (
    <Page layout="collection">
      <PageHeader
        title="Exit export"
        description="All of the laboratory's data in an open, documented format, for leaving this app or keeping an independent copy."
        actions={
          canExport && (
            <Button variant="primary" onPress={() => setExporting(true)}>
              Export all data
            </Button>
          )
        }
      />
      <Toolbar
        variant="collection"
        count={
          query.isSuccess && (
            <ListCount
              shown={rows.length}
              noun="exit exports"
              one="exit export"
              hasMore={hasMore}
            />
          )
        }
      />
      {query.isSuccess && (
        <ListLimit variant="summary" shown={rows.length} end="newest" hasMore={hasMore} />
      )}
      <QueryContent
        query={query}
        what="exit exports"
        loadingRows={4}
        empty={{
          when: (rows) => rows.length === 0,
          title: "No exit exports",
          description: "No one has exported all data from this installation.",
        }}
      >
        {() => (
          <>
            <DataGrid
              label="Exit exports"
              columns={EXIT_EXPORT_COLUMNS}
              rows={rows}
              getRowId={(row) => row.exportId}
              fill
            />
          </>
        )}
      </QueryContent>
      {exporting && <ExitExportDialog onClose={() => setExporting(false)} />}
    </Page>
  );
}
