import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Facts } from "@/components/Facts";
import type { MethodAnalyteView } from "@/generated/MethodAnalyteView";
import type { MethodVersionView } from "@/generated/MethodVersionView";
import { labelOf } from "@/lib/choices";
import { CERTAINTIES, OUTPUT_SCALES, THRESHOLD_ROUNDINGS, THRESHOLD_RULES } from "../labels";
import { RANGE_POLICIES } from "./methodValues";

/** A method version's settings and its analytes' limits, exactly as stored. */
export function MethodContent({ view }: { view: MethodVersionView }) {
  return (
    <>
      <Facts
        layout="compact"
        items={[
          { label: "Report limitations and notes", value: view.reportNotes ?? "Not set" },
          {
            label: "External approval reference",
            value: view.externalApprovalReference ?? "Not recorded",
          },
          { label: "Specimen type", value: view.specimenTypeCode },
          { label: "Certainty", value: labelOf(CERTAINTIES, view.certainty) },
          { label: "Result scale", value: labelOf(OUTPUT_SCALES, view.outputScale) },
          { label: "Cutoff rule", value: labelOf(THRESHOLD_RULES, view.thresholdRule) },
          {
            label: "Cutoff comparison",
            value: labelOf(THRESHOLD_ROUNDINGS, view.thresholdRounding),
          },
        ]}
      />
      <DataGrid
        label={`Analytes of version ${view.version}`}
        columns={ANALYTE_COLUMNS}
        rows={view.analytes}
        getRowId={(analyte) => analyte.analyteId}
        fill
      />
    </>
  );
}

const NOT_SET = "Not set";

/** A decimal exactly as the engine stored it, with the analyte's unit (REQ-ANA-002). */
function quantity(value: string | null | undefined, unit: string | null | undefined): string {
  if (value === null || value === undefined) return NOT_SET;
  return unit ? `${value} ${unit}` : value;
}

function rangeText({ reportableLow, reportableHigh, unit }: MethodAnalyteView): string {
  if (!reportableLow && !reportableHigh) return NOT_SET;
  const low = reportableLow ?? "no lower limit";
  return `${low} to ${reportableHigh ? quantity(reportableHigh, unit) : "no upper limit"}`;
}

function scaleText({ displayScale }: MethodAnalyteView): string {
  return displayScale === null || displayScale === undefined ? NOT_SET : String(displayScale);
}

const ANALYTE_COLUMNS: readonly DataGridColumn<MethodAnalyteView>[] = [
  {
    id: "analyte",
    header: "Analyte",
    cell: (row) => `${row.analyteCode} ${row.analyteName}`,
    sortValue: (row) => row.analyteCode,
  },
  { id: "unit", header: "Unit", cell: (row) => row.unit ?? NOT_SET, width: 100 },
  { id: "cutoff", header: "Cutoff", cell: (row) => quantity(row.cutoff, row.unit), width: 130 },
  { id: "lod", header: "LOD", cell: (row) => quantity(row.limitOfDetection, row.unit), width: 130 },
  {
    id: "loq",
    header: "LOQ",
    cell: (row) => quantity(row.limitOfQuantitation, row.unit),
    width: 130,
  },
  { id: "range", header: "Reportable range", cell: rangeText },
  {
    id: "rangePolicy",
    header: "Range policy",
    cell: (row) => labelOf(RANGE_POLICIES, row.rangePolicy ?? "report_only"),
  },
  {
    id: "reference",
    header: "Reference interval",
    cell: (row) => row.referenceInterval ?? NOT_SET,
  },
  { id: "scale", header: "Display scale", cell: scaleText, width: 120 },
];
