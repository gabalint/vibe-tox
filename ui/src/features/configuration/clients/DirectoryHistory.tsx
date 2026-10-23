import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import type { DirectoryHistoryEntry } from "@/generated/DirectoryHistoryEntry";
import type { DirectoryRecord } from "@/generated/DirectoryRecord";
import { useCommandQuery } from "@/lib/commands";
import { usePersonText } from "@/lib/person";

function facts(row: DirectoryHistoryEntry) {
  const value = row.snapshot;
  const label =
    value.kind === "practitioner"
      ? `${value.npi} · ${value.family_name}, ${value.given_names ?? ""}`
      : `${value.code} · ${value.name}`;
  return `${label} · ${value.active ? "Active" : "Retired"}`;
}
function columns(
  person: (userId: string, display?: string) => string,
): readonly DataGridColumn<DirectoryHistoryEntry>[] {
  return [
    { id: "revision", header: "Revision", cell: (row) => row.revision, width: 90 },
    { id: "facts", header: "Saved facts", cell: facts },
    { id: "at", header: "Recorded", cell: (row) => <Timestamp value={row.recordedAt} /> },
    {
      id: "actor",
      header: "Recorded by",
      cell: (row) =>
        row.recordedBy === null
          ? "Unknown — legacy baseline"
          : person(row.recordedBy, row.recordedByDisplay),
    },
    {
      id: "reason",
      header: "Reason",
      cell: (row) =>
        row.reason ??
        (row.origin === "legacy_baseline"
          ? "Migrated original record; prior changes unknown"
          : "Initial registration"),
    },
  ];
}

/** History is loaded only when the operator explicitly asks for this record's evidence. */
export function DirectoryHistory({ record }: { record: DirectoryRecord }) {
  const [open, setOpen] = useState(false);
  const person = usePersonText();
  const query = useCommandQuery<DirectoryHistoryEntry[]>(
    "directory_history",
    { input: { record } },
    { enabled: open },
  );
  return (
    <>
      <Button onPress={() => setOpen((value) => !value)}>
        {open ? "Hide correction history" : "Show correction history"}
      </Button>
      {open && (
        <QueryContent query={query} what="directory history">
          {(rows) => (
            <DataGrid
              label="Directory correction history"
              columns={columns(person)}
              rows={rows}
              getRowId={(row) => String(row.revision)}
            />
          )}
        </QueryContent>
      )}
    </>
  );
}
