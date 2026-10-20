import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { FindCount } from "@/components/FindField";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { MediaDisposalView } from "@/generated/MediaDisposalView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { BACKUP_RESTORE, LIST_MEDIA_DISPOSALS } from "../contract";
import { DISPOSAL_METHODS } from "../labels";
import { RecordDisposalDialog } from "./RecordDisposalDialog";

const COLUMNS: readonly DataGridColumn<MediaDisposalView>[] = [
  { id: "medium", header: "Medium", cell: (row) => row.medium, sortValue: (row) => row.medium },
  {
    id: "method",
    header: "Method",
    cell: (row) => labelOf(DISPOSAL_METHODS, row.method),
    width: 180,
  },
  { id: "identifier", header: "Identifier", cell: (row) => row.identifier ?? "None" },
  { id: "recordedBy", header: "Recorded by", cell: (row) => row.recordedBy },
  {
    id: "recordedAt",
    header: "Recorded",
    cell: (row) => <Timestamp value={row.recordedAt} />,
    sortValue: (row) => row.recordedAt,
  },
];

/**
 * Disposal of host disks and backup media (REQ-OPS-003): a record of each one wiped,
 * destroyed, re-used after wiping or returned, so the laboratory can show where copies of its
 * data went.
 */
export function MediaDisposalsPage() {
  const query = useCommandQuery<MediaDisposalView[]>(LIST_MEDIA_DISPOSALS);
  const canRecord = useHasCapability(BACKUP_RESTORE);
  const [recording, setRecording] = useState(false);
  return (
    <Page layout="collection">
      <PageHeader
        title="Media disposals"
        description="Records of host disks and backup media that were wiped, destroyed, re-used or returned."
        actions={
          canRecord && (
            <Button variant="primary" onPress={() => setRecording(true)}>
              Record disposal
            </Button>
          )
        }
      />
      <Toolbar
        variant="collection"
        count={
          query.isSuccess && (
            <FindCount
              shown={query.data.length}
              total={query.data.length}
              noun="media disposals"
              one="media disposal"
            />
          )
        }
      />
      <QueryContent
        query={query}
        what="media disposals"
        loadingRows={8}
        empty={{
          when: (rows) => rows.length === 0,
          title: "No disposals recorded",
          description:
            "Record each disk or backup medium that leaves the laboratory's use, and how its data was removed.",
        }}
      >
        {(rows) => (
          <DataGrid
            label="Media disposals"
            columns={COLUMNS}
            rows={rows}
            getRowId={(row) => row.disposalId}
            fill
          />
        )}
      </QueryContent>
      {recording && <RecordDisposalDialog onClose={() => setRecording(false)} />}
    </Page>
  );
}
