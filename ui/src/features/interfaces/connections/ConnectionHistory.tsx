import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Timestamp } from "@/components/Timestamp";
import type { ConnectionChangeView } from "@/generated/ConnectionChangeView";
import { usePersonText } from "@/lib/person";
import { CHANGES } from "../labels";

/** The settings shown when a change names them. */
const FIELD_LABELS: Record<string, string> = {
  name: "name",
  partner: "partner",
  folder: "folder",
  rules: "rules",
  schedule: "schedule",
  fileNamePattern: "file name",
  active: "started or stopped",
};

const fieldsText = (entry: ConnectionChangeView) =>
  entry.fields.map((field) => FIELD_LABELS[field] ?? field).join(", ");

/** Every change of the connection and its rules, newest first, with who, when and why. */
export function ConnectionHistory({ history }: { history: readonly ConnectionChangeView[] }) {
  const person = usePersonText();
  const columns: readonly DataGridColumn<ConnectionChangeView>[] = [
    {
      id: "at",
      header: "When",
      cell: (row) => <Timestamp value={row.changed.at} />,
      sortValue: (row) => row.changed.at,
      defaultSort: "desc",
      width: 190,
    },
    {
      id: "who",
      header: "Who",
      cell: (row) => person(row.changed.by, row.changed.byDisplay),
      width: 180,
    },
    { id: "change", header: "Change", cell: (row) => CHANGES[row.change], width: 210 },
    { id: "fields", header: "Settings changed", cell: fieldsText, wrap: true },
    { id: "reason", header: "Reason or method", cell: (row) => row.reason ?? "", wrap: true },
  ];
  return (
    <DataGrid
      label="Connection history"
      columns={columns}
      rows={history}
      getRowId={(row) => String(row.revision)}
    />
  );
}
