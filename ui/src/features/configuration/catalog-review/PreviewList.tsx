import type { UseQueryResult } from "@tanstack/react-query";
import { DataGrid, type GridRowActions } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { FindCount, FindField, findRows, NoResults } from "@/components/FindField";
import { QueryContent } from "@/components/QueryContent";
import type { ActionItem } from "@/components/RowActions";
import { Toolbar } from "@/components/Toolbar";
import type { CatalogPreviewSummary } from "@/generated/CatalogPreviewSummary";
import type { CommandError } from "@/generated/CommandError";
import { useFindParam } from "@/lib/findParam";
import type { OpenRecord } from "@/lib/openRecord";
import { PREVIEW_COLUMNS, PreviewPanel } from "./PreviewPanel";

const getRowId = (preview: CatalogPreviewSummary) => preview.id;
const findFields = (preview: CatalogPreviewSummary) => [
  preview.id,
  preview.generatedByDisplay ?? preview.generatedBy,
  preview.reviewedByDisplay ?? preview.reviewedBy,
];

interface PreviewListProps {
  previews: UseQueryResult<CatalogPreviewSummary[], CommandError>;
  record: OpenRecord;
  actionsOf: (preview: CatalogPreviewSummary) => ActionItem[];
  onOpen: (id: string, order?: readonly string[]) => void;
}

/** One report history, filtered locally; opening a row shows its stored PDF and review. */
export function PreviewList({ previews, record, actionsOf, onOpen }: PreviewListProps) {
  const [query, setQuery] = useFindParam();
  const rows = previews.data ?? [];
  const shown = findRows(rows, query, findFields);
  const open = rows.find((row) => row.id === record.openId);
  const rowActions: GridRowActions<CatalogPreviewSummary> = {
    label: getRowId,
    hasItems: () => true,
    items: actionsOf,
  };
  return (
    <DetailLayout
      panel={open && <PreviewPanel preview={open} actions={actionsOf(open)} record={record} />}
    >
      <Toolbar
        variant="collection"
        count={
          previews.data &&
          !previews.isError && (
            <FindCount shown={shown.length} total={rows.length} noun="reports" one="report" />
          )
        }
      >
        <FindField
          label="Find catalog previews"
          placeholder="Report ID or person"
          value={query}
          onChange={setQuery}
        />
      </Toolbar>
      <QueryContent
        query={previews}
        what="Catalog Preview reports"
        loadingRows={3}
        empty={{
          when: (data) => data.length === 0,
          title: "No Catalog Preview yet",
          description:
            "Generate a report to review catalog values entered manually or loaded from starter data.",
        }}
      >
        {() =>
          shown.length === 0 ? (
            <NoResults noun="catalog previews" query={query} onClear={() => setQuery("")} />
          ) : (
            <DataGrid
              label="Catalog Preview reports"
              columns={PREVIEW_COLUMNS}
              rows={shown}
              getRowId={getRowId}
              openRowId={record.openId}
              onRowAction={(row, order) => onOpen(row.id, order)}
              onOrderChange={record.followOrder}
              rowActions={rowActions}
              fill
            />
          )
        }
      </QueryContent>
    </DetailLayout>
  );
}
