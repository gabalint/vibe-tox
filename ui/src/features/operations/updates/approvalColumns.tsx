import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import type { Status } from "../labels";
import { Person } from "../Person";

/** The engine's facts about an approval, in words; the engine still decides a withdrawal. */
function approvalStatus(row: UpdateApprovalView): Status {
  if (row.withdrawn) return { label: "Withdrawn", tone: "neutral" };
  if (row.installed) return { label: "Installed", tone: "neutral" };
  return { label: "Not yet installed", tone: "info" };
}

export function ApprovalBadge({ approval }: { approval: UpdateApprovalView }) {
  const status = approvalStatus(approval);
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}

/**
 * Whether the approval can still be withdrawn: until a run of its version starts. Engine
 * request: `allowedActions` on `UpdateApprovalView` would replace this condition.
 */
export const isPending = (row: UpdateApprovalView) => row.withdrawn === null && !row.installed;

export const APPROVAL_COLUMNS: readonly DataGridColumn<UpdateApprovalView>[] = [
  {
    id: "version",
    header: "Version",
    identifier: true,
    cell: (row) => row.version,
    sortValue: (row) => row.version,
    // Wide enough for a nightly version such as 0.1.2-nightly.20261028.4.
    width: 240,
  },
  {
    id: "status",
    header: "Status",
    cell: (row) => <ApprovalBadge approval={row} />,
    width: 170,
  },
  { id: "risk", header: "Risk class", cell: (row) => row.riskClassText },
  { id: "package", header: "Package file", cell: (row) => row.packageFileName },
  {
    id: "approved",
    header: "Approved",
    cell: (row) => (
      <>
        <Timestamp value={row.approvedAt} /> by{" "}
        <Person id={row.approvedBy} display={row.approvedByDisplay} />
      </>
    ),
    sortValue: (row) => row.approvedAt,
  },
];
