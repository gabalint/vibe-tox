import { DataGrid, type DataGridColumn, type GridRowActions } from "@/components/DataGrid";
import { Timestamp } from "@/components/Timestamp";
import type { ObligationView } from "../contract";
import { ORIGIN, sendsText } from "../labels";

function lastAttempt(row: ObligationView) {
  const last = row.attempts.at(-1);
  return last ? <Timestamp value={last.observedAt} /> : "None";
}

const BASE_COLUMNS: readonly DataGridColumn<ObligationView>[] = [
  // REQ-RPT-011: the engine's label names the channel and the evidence level, and its length
  // varies (an attestation names who and when), so it wraps rather than being cut.
  { id: "status", header: "Delivery status", cell: (row) => row.status.label, wrap: true },
  { id: "destination", header: "Destination", cell: (row) => row.destination.label, width: 170 },
  { id: "item", header: "Sends", cell: sendsText, width: 130, wrap: true },
  {
    id: "origin",
    header: "Why",
    cell: (row) =>
      row.originReason ? `${ORIGIN[row.origin]}: ${row.originReason}` : ORIGIN[row.origin],
    width: 150,
    wrap: true,
  },
  { id: "last", header: "Last attempt", cell: lastAttempt, width: 150, wrap: true },
];

const WITH_REPORT: readonly DataGridColumn<ObligationView>[] = [
  {
    id: "report",
    header: "Report",
    cell: (row) => row.reportNumber,
    sortValue: (row) => row.reportNumber,
    width: 140,
    identifier: true,
  },
  ...BASE_COLUMNS,
];

const obligationId = (row: ObligationView) => row.obligationId;

interface ObligationGridProps {
  obligations: readonly ObligationView[];
  /** The actions the engine allows on each row (`useObligationActions`). */
  rowActions: GridRowActions<ObligationView>;
  /** Opens a row's obligation, given the rows' IDs in display order. */
  onOpen: (row: ObligationView, order: readonly string[]) => void;
  /** The obligation open in the delivery queue's side panel. */
  openId?: string | null;
  /** The panel's `followOrder`, so its Previous and Next follow a sort. */
  onOrderChange?: (order: readonly string[]) => void;
  /**
   * The delivery queue lists several reports, so the report column, the rows' identifier,
   * comes first. A report's own page lists only its obligations, without it.
   */
  withReport?: boolean;
  /** The grid takes the remaining height of the page (the main grid of a list page). */
  fill?: boolean;
}

/**
 * Delivery obligations (REQ-RPT-003, REQ-RPT-011): each status as the engine words it,
 * naming the channel and the evidence, with each row's allowed actions in its menu.
 */
export function ObligationGrid({ obligations, withReport, onOpen, ...props }: ObligationGridProps) {
  return (
    <DataGrid
      label="Delivery obligations"
      columns={withReport ? WITH_REPORT : BASE_COLUMNS}
      rows={obligations}
      getRowId={obligationId}
      onRowAction={onOpen}
      openRowId={props.openId}
      onOrderChange={props.onOrderChange}
      rowActions={props.rowActions}
      fill={props.fill}
    />
  );
}
