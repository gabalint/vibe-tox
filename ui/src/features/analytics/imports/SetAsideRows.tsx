import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import type { SetAsideRowView } from "@/generated/SetAsideRowView";
import styles from "@/styles/layout.module.css";
import { quarantineReasonText } from "../resultLabels";

/** Why a row is listed without a quarantine item (result files brief F9, F17). */
function setAsideText(row: SetAsideRowView): string {
  if (row.kind === "batch_row_not_placed") {
    return "Calibrator or blank row of the batch, not on a run";
  }
  const reason = row.reason === null ? "" : `: ${quarantineReasonText(row.reason)}`;
  return `Measured, not ordered, not kept${reason}`;
}

/** Source text is shown as plain text, exactly as received; empty cells are said. */
const COLUMNS: readonly DataGridColumn<SetAsideRowView>[] = [
  {
    id: "record",
    header: "Record",
    cell: (row) => <span className={styles.code}>{row.recordPath}</span>,
    width: 140,
  },
  { id: "sample", header: "Sample label", cell: (row) => row.sampleLabel ?? "Empty", width: 150 },
  { id: "code", header: "Analyte as written", cell: (row) => row.testCode ?? "Empty", width: 170 },
  { id: "value", header: "Value as received", cell: (row) => row.valueText ?? "Empty", width: 150 },
  { id: "type", header: "Sample type", cell: (row) => row.typeWord ?? "None", width: 130 },
  { id: "why", header: "Why set aside", cell: setAsideText, wrap: true },
];

/**
 * File rows an import attempt lists without quarantining them (F9, F17): calibrator and
 * blank rows of the batch that are not on a run, and rows measured outside the ordered scope
 * that failed a check. They need no decision.
 */
export function SetAsideRows({ rows }: { rows: readonly SetAsideRowView[] }) {
  return (
    <>
      <h2 className={styles.subheading}>Rows set aside</h2>
      {rows.length === 0 ? (
        <p className={styles.hint}>No rows were set aside.</p>
      ) : (
        <DataGrid
          label="Rows set aside"
          columns={COLUMNS}
          rows={rows}
          getRowId={(row) => row.rowId}
          fill
        />
      )}
    </>
  );
}
