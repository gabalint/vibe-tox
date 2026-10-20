import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";
import { type SyntheticRow, syntheticRowActions, syntheticRows } from "./syntheticRows";

const ROW_COUNT = 10_000;

/** Synthetic worklist columns, also used by the side panel demo. */
export const SYNTHETIC_COLUMNS: readonly DataGridColumn<SyntheticRow>[] = [
  {
    id: "id",
    header: "Accession",
    cell: (row) => row.id,
    sortValue: (row) => row.id,
    width: 120,
    identifier: true,
  },
  { id: "panel", header: "Panel", cell: (row) => row.panel, sortValue: (row) => row.panel },
  {
    id: "status",
    header: "Status",
    cell: (row) => <StatusBadge tone={row.status.tone}>{row.status.label}</StatusBadge>,
    sortValue: (row) => row.status.label,
    width: 140,
  },
  {
    id: "results",
    header: "Results",
    cell: (row) => row.results,
    sortValue: (row) => row.results,
    align: "end",
    width: 96,
  },
  {
    id: "received",
    header: "Received",
    cell: (row) => row.received,
    sortValue: (row) => row.received,
    width: 150,
  },
];

/** The same rows with a wrapping note column, in a short box that the grid fills. */
const NOTE_COLUMNS: readonly DataGridColumn<SyntheticRow>[] = [
  { id: "id", header: "Accession", cell: (row) => row.id, width: 120 },
  { id: "note", header: "Note", cell: (row) => row.note, wrap: true, width: 280 },
  { id: "panel", header: "Panel", cell: (row) => row.panel },
  { id: "results", header: "Results", cell: (row) => row.results, align: "end", width: 96 },
];

const getRowId = (row: SyntheticRow) => row.id;

/**
 * 10,000 synthetic rows to check scrolling, sorting, opening rows, row actions and bulk
 * selection on each web engine (NFR-33), and a filling grid with a wrapping column.
 */
export function GridDemo() {
  const rows = useMemo(() => syntheticRows(ROW_COUNT), []);
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const [opened, setOpened] = useState<string | null>(null);
  const [lastAction, setLastAction] = useState("none");
  const rowActions = useMemo(() => syntheticRowActions(setLastAction), []);
  return (
    <GallerySection title="Data grid: 10,000 synthetic rows">
      <p role="status">
        Last opened: {opened ?? "none"}. Last action: {lastAction}.
      </p>
      <DataGrid
        label="Synthetic worklist"
        columns={SYNTHETIC_COLUMNS}
        rows={rows}
        getRowId={getRowId}
        onRowAction={(row) => setOpened(row.id)}
        openRowId={opened}
        rowActions={rowActions}
        bulk={{
          selectedIds,
          onChange: setSelectedIds,
          actions: (
            <Button onPress={() => setLastAction(`handed off ${selectedIds.size} rows`)}>
              Hand off
            </Button>
          ),
        }}
      />
      <div className={styles.fillBox}>
        <p>This grid fills the rest of the box, and its Note column wraps.</p>
        <DataGrid
          label="Synthetic notes"
          columns={NOTE_COLUMNS}
          rows={rows}
          getRowId={getRowId}
          fill
        />
      </div>
    </GallerySection>
  );
}
