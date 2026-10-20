import { useMemo } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount, ListLimit, withinLimit } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { AnchorExportView } from "@/generated/AnchorExportView";
import type { ListAnchorExportsInput } from "@/generated/ListAnchorExportsInput";
import { useCommandQuery } from "@/lib/commands";
import { streamText } from "./anchorText";
import { LIST_ANCHOR_EXPORTS, LIST_LIMIT } from "./contract";

// One more than shown, so the list can say that more exist.
const LIST_INPUT: ListAnchorExportsInput = { limit: LIST_LIMIT + 1 };

function rangesText(row: AnchorExportView): string {
  return row.ranges
    .map((range) => `${streamText(range.stream)}: seals ${range.firstSealNo}–${range.lastSealNo}`)
    .join("; ");
}

const EXPORT_COLUMNS: readonly DataGridColumn<AnchorExportView>[] = [
  {
    id: "target",
    header: "Target",
    cell: (row) => row.targetLabel,
    sortValue: (row) => row.targetLabel,
  },
  { id: "file", header: "File name", cell: (row) => row.fileName },
  {
    id: "exportedAt",
    header: "Exported",
    cell: (row) => <Timestamp value={row.exportedAt} />,
    sortValue: (row) => row.exportedAt,
    width: 190,
  },
  { id: "ranges", header: "Seals", cell: rangesText },
];

/** The anchor files written to each target, newest first. Files are never changed afterwards. */
export function AnchorExportsSection() {
  const exports = useCommandQuery<AnchorExportView[]>(LIST_ANCHOR_EXPORTS, {
    input: LIST_INPUT,
  });
  return (
    <QueryContent
      query={exports}
      what="anchor files"
      loadingRows={4}
      empty={{
        when: (rows) => rows.length === 0,
        title: "No anchor files yet",
        description: "Anchors are written to every available backup target after each seal.",
      }}
    >
      {(answer) => <ExportList answer={answer} />}
    </QueryContent>
  );
}

function ExportList({ answer }: { answer: readonly AnchorExportView[] }) {
  const { rows, hasMore } = useMemo(() => withinLimit(answer, LIST_LIMIT), [answer]);
  return (
    <>
      <Toolbar
        variant="collection"
        count={
          <ListCount shown={rows.length} noun="anchor files" one="anchor file" hasMore={hasMore} />
        }
      />
      <ListLimit shown={rows.length} end="newest" hasMore={hasMore} variant="summary" />
      <DataGrid
        label="Anchor files"
        columns={EXPORT_COLUMNS}
        rows={rows}
        getRowId={(row) => row.exportId}
      />
    </>
  );
}
