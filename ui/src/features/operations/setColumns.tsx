import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { BackupSetView } from "@/generated/BackupSetView";
import { backupKind, sizeText } from "./labels";

function compatibility(set: BackupSetView) {
  if (set.compatible) return <StatusBadge tone="neutral">Compatible</StatusBadge>;
  const reason = set.incompatibleReason ? `: ${set.incompatibleReason}` : "";
  return <StatusBadge tone="danger">{`Not compatible${reason}`}</StatusBadge>;
}

/** Columns of a list of backup sets, as the engine reads them from their manifests. */
export const SET_COLUMNS: readonly DataGridColumn<BackupSetView>[] = [
  {
    id: "createdAt",
    header: "Backup made",
    cell: (set) => <Timestamp value={set.createdAt} />,
    sortValue: (set) => set.createdAt,
    width: 180,
  },
  { id: "kind", header: "Kind", cell: (set) => backupKind(set.kind), width: 140 },
  { id: "appVersion", header: "App version", cell: (set) => set.appVersion, width: 110 },
  {
    id: "databaseBytes",
    header: "Size",
    cell: (set) => sizeText(set.databaseBytes),
    align: "end",
    width: 100,
  },
  {
    id: "artifactCount",
    header: "Files",
    cell: (set) => set.artifactCount,
    align: "end",
    width: 80,
  },
  { id: "compatibility", header: "Compatibility", cell: compatibility, width: 240 },
];
