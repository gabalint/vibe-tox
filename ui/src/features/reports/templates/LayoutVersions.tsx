import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { LayoutVersionView } from "@/generated/LayoutVersionView";
import { usePersonText } from "@/lib/person";
import { versionStates } from "./labels";

function columns(person: (userId: string, display?: string) => string) {
  const list: readonly DataGridColumn<LayoutVersionView>[] = [
    {
      id: "version",
      header: "Version",
      cell: (row) => `Version ${row.versionNo}`,
      sortValue: (row) => row.versionNo,
      identifier: true,
      width: 110,
    },
    { id: "layout", header: "Layout", cell: (row) => row.name, width: 140 },
    {
      id: "state",
      header: "Status",
      cell: (row) => {
        const state = versionStates[row.state];
        return <StatusBadge tone={state.tone}>{state.label}</StatusBadge>;
      },
      width: 190,
    },
    {
      id: "drafted",
      header: "Drafted",
      cell: (row) => (
        <>
          {person(row.draftedBy.userId, row.draftedBy.displayName)},{" "}
          <Timestamp value={row.draftedAt} />
        </>
      ),
    },
    {
      id: "decided",
      header: "Approved or rejected",
      cell: (row) =>
        row.decidedBy && row.decidedAt ? (
          <>
            {person(row.decidedBy.userId, row.decidedBy.displayName)},{" "}
            <Timestamp value={row.decidedAt} />
          </>
        ) : (
          "Not yet"
        ),
    },
    { id: "reason", header: "Rejection reason", cell: (row) => row.rejectionReason ?? "" },
  ];
  return list;
}

/** Every layout version, newest first: drafts, approvals, rejections and superseded layouts. */
export function LayoutVersions({ versions }: { versions: readonly LayoutVersionView[] }) {
  const person = usePersonText();
  return (
    <DataGrid
      label="Layout history"
      columns={columns(person)}
      rows={versions}
      getRowId={(row) => row.versionId}
    />
  );
}
