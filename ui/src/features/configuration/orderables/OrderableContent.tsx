import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import type { CatalogComponentView } from "@/generated/CatalogComponentView";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import { labelOf } from "@/lib/choices";
import { CERTAINTIES, OUTPUT_SCALES, PURPOSES } from "../labels";

/** A test or panel version's components, each with its purpose (REQ-SPC-004). */
export function OrderableContent({ view }: { view: OrderableVersionView }) {
  return (
    <DataGrid
      label={`Components of version ${view.version}`}
      columns={COMPONENT_COLUMNS}
      rows={view.components}
      getRowId={(component) => String(component.position)}
      fill
    />
  );
}

const COMPONENT_COLUMNS: readonly DataGridColumn<CatalogComponentView>[] = [
  { id: "position", header: "Position", cell: (row) => row.position, width: 100, align: "end" },
  {
    id: "purpose",
    header: "Purpose",
    cell: (row) => labelOf(PURPOSES, row.purpose),
    sortValue: (row) => row.purpose,
    width: 200,
  },
  {
    id: "method",
    header: "Method version",
    cell: (row) => `${row.methodCode} version ${row.methodVersion}`,
  },
  { id: "specimen", header: "Specimen type", cell: (row) => row.specimenTypeCode, width: 140 },
  {
    id: "certainty",
    header: "Certainty",
    cell: (row) => labelOf(CERTAINTIES, row.certainty),
  },
  { id: "scale", header: "Result scale", cell: (row) => labelOf(OUTPUT_SCALES, row.outputScale) },
  {
    id: "analytes",
    header: "Analytes reported",
    cell: (row) => row.analytes.map((analyte) => analyte.code).join(", "),
  },
];
