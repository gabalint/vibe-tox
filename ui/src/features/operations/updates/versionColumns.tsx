import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { InstalledVersionView } from "@/generated/InstalledVersionView";
import type { Stamp } from "@/generated/Stamp";
import type { VersionStatus as Code } from "@/generated/VersionStatus";
import type { Status } from "../labels";
import { Person } from "../Person";

const VERSION_STATUSES: Record<Code, Status> = {
  accepted: { label: "Accepted", tone: "neutral" },
  awaiting_acceptance: { label: "Awaiting acceptance", tone: "warning" },
  unapproved: { label: "Started without approval", tone: "danger" },
  previous: { label: "Previous", tone: "neutral" },
};

/** The engine's status of an installed version; an unknown code shows as received. */
export function VersionStatus({ status }: { status: string }) {
  const known = Object.entries(VERSION_STATUSES).find(([code]) => code === status);
  const shown: Status = known?.[1] ?? { label: status, tone: "neutral" };
  return <StatusBadge tone={shown.tone}>{shown.label}</StatusBadge>;
}

export function Decision({ decision }: { decision: Stamp | null }) {
  if (!decision) return "None";
  return (
    <>
      <Timestamp value={decision.at} /> by <Person id={decision.by} />
    </>
  );
}

export const VERSION_COLUMNS: readonly DataGridColumn<InstalledVersionView>[] = [
  {
    id: "version",
    header: "Version",
    identifier: true,
    cell: (row) => row.version,
    width: 110,
  },
  {
    id: "firstStarted",
    header: "First started",
    cell: (row) => <Timestamp value={row.firstStartedAt} />,
    sortValue: (row) => row.firstStartedAt,
  },
  {
    id: "lastStarted",
    header: "Last started",
    cell: (row) => <Timestamp value={row.lastStartedAt} />,
    sortValue: (row) => row.lastStartedAt,
  },
  { id: "running", header: "Running", cell: (row) => (row.running ? "Yes" : "No"), width: 90 },
  {
    id: "status",
    header: "Status",
    cell: (row) => <VersionStatus status={row.status} />,
    sortValue: (row) => row.status,
    width: 220,
  },
  {
    id: "approval",
    header: "Approved",
    cell: (row) => (
      <Decision
        decision={row.approval && { by: row.approval.approvedBy, at: row.approval.approvedAt }}
      />
    ),
  },
  {
    id: "acceptance",
    header: "Accepted for clinical use",
    cell: (row) => <Decision decision={row.acceptance} />,
  },
];
