import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { ExitExportView } from "@/generated/ExitExportView";
import styles from "@/styles/layout.module.css";
import { jobState, sizeText } from "../labels";
import { Person } from "../Person";

function State({ state }: { state: string }) {
  const shown = jobState(state);
  return <StatusBadge tone={shown.tone}>{shown.label}</StatusBadge>;
}

export const EXIT_EXPORT_COLUMNS: readonly DataGridColumn<ExitExportView>[] = [
  {
    id: "requested",
    header: "Requested",
    cell: (row) => <Timestamp value={row.requestedAt} />,
    sortValue: (row) => row.requestedAt,
    width: 190,
  },
  { id: "by", header: "By", cell: (row) => <Person id={row.requestedBy} /> },
  { id: "state", header: "State", cell: (row) => <State state={row.state} />, width: 130 },
  { id: "folder", header: "Folder", cell: (row) => row.folderLabel },
  { id: "package", header: "Package", cell: (row) => row.packageName ?? "Not written" },
  {
    id: "size",
    header: "Database size",
    cell: (row) => sizeText(row.databaseBytes),
    sortValue: (row) => row.databaseBytes ?? -1,
    align: "end",
    width: 120,
  },
  {
    id: "files",
    header: "Stored files",
    cell: (row) => (row.artifactCount === null ? "Unknown" : String(row.artifactCount)),
    align: "end",
    width: 110,
  },
  {
    id: "manifest",
    header: "Manifest SHA-256",
    cell: (row) =>
      row.manifestSha256 ? <span className={styles.code}>{row.manifestSha256}</span> : "None",
  },
  { id: "failure", header: "Failure", cell: (row) => row.failureCode ?? "None", width: 160 },
];
