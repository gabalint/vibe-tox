import type { GridRowActions } from "@/components/DataGrid";
import type { ActionItem } from "@/components/RowActions";
import type { Tone } from "@/components/tones";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";

/** A synthetic worklist row for the grid demo. No personal or clinical data. */
export interface SyntheticRow {
  id: string;
  panel: string;
  status: { label: string; tone: Tone };
  received: string;
  /** A count, zero on some rows. */
  results: number;
  /** Free text, long on some rows, for the wrapping column. */
  note: string;
  /** Stands in for the engine's `allowedActions`: finished rows have none. */
  allowedActions: readonly string[];
}

const PANELS = ["Synthetic panel A", "Synthetic panel B", "Synthetic panel C"];
const STATUSES = [
  { label: "Received", tone: "info" },
  { label: "In progress", tone: "neutral" },
  { label: "On hold", tone: "hold" },
  { label: "Reviewed", tone: "neutral" },
] as const;

const NOTES = [
  "Ready",
  "Waiting for a synthetic confirmation result before the specimen can move on to review",
  "Synthetic note",
];

const pad = (value: number, width: number) => String(value).padStart(width, "0");

/** Deterministic rows `SYN-000001` … for responsiveness checks (NFR-33). */
export function syntheticRows(count: number): SyntheticRow[] {
  return Array.from({ length: count }, (_, index) => {
    const minutes = index % (24 * 60);
    return {
      id: `SYN-${pad(index + 1, 6)}`,
      panel: PANELS[index % PANELS.length] ?? "",
      status: STATUSES[index % STATUSES.length] ?? STATUSES[0],
      received: `2026-10-12 ${pad(Math.floor(minutes / 60), 2)}:${pad(minutes % 60, 2)}`,
      results: index % 5 === 0 ? 0 : (index % 48) + 1,
      note: NOTES[index % NOTES.length] ?? "",
      allowedActions: index % STATUSES.length === 3 ? [] : ["review", "discard"],
    };
  });
}

/** The synthetic actions, in the order the record view and the row menu show them. */
const ACTIONS: readonly OfferedAction<"review" | "discard">[] = [
  { id: "review", label: "Mark reviewed" },
  { id: "discard", label: "Discard row", variant: "danger" },
];

const done = { review: "reviewed", discard: "discarded" };

/** The row's allowed synthetic actions; `onAction` reports what was chosen. */
export function syntheticActions(
  row: SyntheticRow,
  onAction: (text: string) => void,
): ActionItem[] {
  return allowedActionItems(ACTIONS, row.allowedActions, (id) => onAction(`${row.id} ${done[id]}`));
}

/** The grid's `rowActions` for synthetic rows; `onAction` reports what was chosen. */
export function syntheticRowActions(
  onAction: (text: string) => void,
): GridRowActions<SyntheticRow> {
  return allowedRowActions<SyntheticRow, "review" | "discard">(ACTIONS, {
    label: (row) => row.id,
    allowed: (row) => row.allowedActions,
    onAction: (id, row) => onAction(`${row.id} ${done[id]}`),
  });
}
