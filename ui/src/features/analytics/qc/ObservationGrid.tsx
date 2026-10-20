import { useMemo } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { QcObservationView } from "@/generated/QcObservationView";
import type { RunQcView } from "@/generated/RunQcView";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";

/** An observation with the code of the analyte its test code maps to, if any. */
interface ObservationRow extends QcObservationView {
  analyteCode: string | null;
}

const NOT_MAPPED = "Not mapped";

/** Values, units, flags and status are the analyzer's text, shown exactly as received. */
function observationColumns(
  lotName: (lotId: string | null) => string,
): DataGridColumn<ObservationRow>[] {
  return [
    {
      id: "analyte",
      header: "Analyte",
      cell: (row) => row.analyteCode ?? NOT_MAPPED,
      sortValue: (row) => row.analyteCode ?? NOT_MAPPED,
      width: 130,
    },
    {
      id: "position",
      header: "Position",
      cell: (row) => row.positionLabel,
      sortValue: (row) => row.positionLabel,
      width: 110,
    },
    { id: "lot", header: "Lot", cell: (row) => lotName(row.lotId) },
    {
      id: "code",
      header: "Test code",
      cell: (row) => <span className={styles.code}>{row.sourceTestCode}</span>,
      width: 120,
    },
    {
      id: "value",
      header: "Value",
      cell: receivedValueText,
      wrap: true,
    },
    { id: "flags", header: "Flags", cell: (row) => row.flagsText ?? "—", width: 100 },
    { id: "status", header: "Status", cell: (row) => row.statusText ?? "—", width: 100 },
    {
      id: "current",
      header: "Current",
      cell: (row) =>
        row.superseded ? (
          <StatusBadge tone="neutral">Replaced</StatusBadge>
        ) : (
          <StatusBadge tone="neutral">Current</StatusBadge>
        ),
      width: 130,
    },
    {
      id: "recorded",
      header: "Received",
      cell: (row) => <Timestamp value={row.recordedAt} />,
      width: 200,
    },
  ];
}

interface ObservationGridProps {
  view: RunQcView;
  lotName: (lotId: string | null) => string;
}

/**
 * Every QC observation of the run in one grid (REQ-ANA-004), analyte by analyte in the
 * engine's order, then those whose test code maps to no analyte. Sort by any column to
 * compare across analytes.
 */
export function ObservationGrid({ view, lotName }: ObservationGridProps) {
  const columns = useMemo(() => observationColumns(lotName), [lotName]);
  const rows = useMemo(
    () => [
      ...view.analytes.flatMap((analyte) =>
        analyte.observations.map((observation) => ({ ...observation, analyteCode: analyte.code })),
      ),
      ...view.unmappedObservations.map((observation) => ({ ...observation, analyteCode: null })),
    ],
    [view],
  );
  return (
    <>
      <Toolbar
        variant="collection"
        count={<ListCount shown={rows.length} noun="QC observations" one="QC observation" />}
      />
      <DataGrid
        label="QC observations"
        columns={columns}
        rows={rows}
        getRowId={(row) => row.observationId}
        fill
      />
    </>
  );
}
