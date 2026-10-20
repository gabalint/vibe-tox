import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import type { ReportCompleteness, RevisionAvailability } from "../contract";
import { AVAILABILITY, COMPLETENESS } from "../labels";

interface WithStatus {
  availability: RevisionAvailability;
  completeness: ReportCompleteness;
}

/** The availability and completeness columns that report and revision lists share. */
export function statusColumns<Row extends WithStatus>(): DataGridColumn<Row>[] {
  return [
    {
      id: "availability",
      header: "Availability",
      cell: (row) => (
        <StatusBadge tone={AVAILABILITY[row.availability].tone}>
          {AVAILABILITY[row.availability].label}
        </StatusBadge>
      ),
      width: 140,
    },
    {
      id: "completeness",
      header: "Completeness",
      cell: (row) => COMPLETENESS[row.completeness].label,
      width: 130,
    },
  ];
}
