import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { useWidePanel } from "@/components/DetailPanel";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import type { CommandName } from "@/generated/CommandName";
import { useCommandQuery } from "@/lib/commands";
import { usePersonText } from "@/lib/person";

/** The columns every immutable correction history shares. */
export interface CorrectionHistoryRow {
  revision: number;
  recordedAt: string;
  recordedBy: string | null;
  recordedByDisplay?: string;
  reason: string | null;
  origin: string;
}

function columns<Row extends CorrectionHistoryRow>(
  facts: (row: Row) => string,
  person: (userId: string, display?: string) => string,
): readonly DataGridColumn<Row>[] {
  return [
    { id: "revision", header: "Revision", cell: (row) => row.revision, width: 90 },
    { id: "facts", header: "Saved facts", cell: facts, wrap: true },
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
      wrap: true,
      cell: (row) =>
        row.reason ??
        (row.origin === "legacy_baseline"
          ? "Migrated original record; prior changes unknown"
          : "Initial registration"),
    },
  ];
}

interface CorrectionHistoryProps<Row extends CorrectionHistoryRow> {
  /** The history query and its input for one record. */
  command: CommandName;
  input: object;
  /** The grid's accessible name. */
  label: string;
  /** One line with the facts saved at that revision. */
  facts: (row: Row) => string;
}

/**
 * History is loaded only when the operator explicitly asks for this record's evidence. In a side
 * panel, the panel widens while it is shown, so the table's columns fit.
 */
export function CorrectionHistory<Row extends CorrectionHistoryRow>({
  command,
  input,
  label,
  facts,
}: CorrectionHistoryProps<Row>) {
  const [open, setOpen] = useState(false);
  useWidePanel(open);
  const person = usePersonText();
  const query = useCommandQuery<Row[]>(command, { input }, { enabled: open });
  return (
    <>
      <Button onPress={() => setOpen((value) => !value)}>
        {open ? "Hide correction history" : "Show correction history"}
      </Button>
      {open && (
        <QueryContent query={query} what="correction history">
          {(rows) => (
            <DataGrid
              label={label}
              columns={columns(facts, person)}
              rows={rows}
              getRowId={(row) => String(row.revision)}
            />
          )}
        </QueryContent>
      )}
    </>
  );
}
