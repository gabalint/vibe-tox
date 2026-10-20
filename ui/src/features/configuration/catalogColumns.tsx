import type { DataGridColumn } from "@/components/DataGrid";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { MethodView } from "@/generated/MethodView";
import type { OrderableView } from "@/generated/OrderableView";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import { labelOf } from "@/lib/choices";
import { ORDERABLE_KINDS } from "./labels";
import { type VersionNumbers, VersionState, versionStateText } from "./VersionState";

/**
 * Code and name, the columns every coded catalog record has. The code is the row's
 * identifier, styled as a link: a click on the row or Enter opens the record.
 */
export function codedColumns<Row extends { code: string; name: string }>(): DataGridColumn<Row>[] {
  return [
    {
      id: "code",
      header: "Code",
      cell: (row) => row.code,
      sortValue: (row) => row.code,
      width: 180,
      identifier: true,
    },
    { id: "name", header: "Name", cell: (row) => row.name, sortValue: (row) => row.name },
  ];
}

/** The engine's version in effect and draft awaiting approval, under `header`. */
function versionColumn<Row extends VersionNumbers>(header: string): DataGridColumn<Row> {
  return {
    id: "versions",
    header,
    cell: (row) => <VersionState record={row} />,
    sortValue: versionStateText,
    width: 280,
    wrap: true,
  };
}

/** "Not provided" for a code the laboratory left out, never a blank cell. */
export const notProvided = (value: string | null) => value ?? "Not provided";

export const ANALYTE_COLUMNS: readonly DataGridColumn<AnalyteView>[] = [
  ...codedColumns<AnalyteView>(),
  { id: "loinc", header: "LOINC", cell: (row) => notProvided(row.loincCode), width: 140 },
  { id: "class", header: "Drug class", cell: (row) => notProvided(row.drugClassCode), width: 160 },
];

export const specimenTypePath = (id: string) => `/configuration/specimen-types/${id}`;
export const methodPath = (id: string) => `/configuration/methods/${id}`;
export const orderablePath = (id: string) => `/configuration/orderables/${id}`;

export const SPECIMEN_TYPE_COLUMNS: readonly DataGridColumn<SpecimenTypeView>[] = [
  ...codedColumns<SpecimenTypeView>(),
  versionColumn("Validity policy"),
];

export const METHOD_COLUMNS: readonly DataGridColumn<MethodView>[] = [
  ...codedColumns<MethodView>(),
  versionColumn("Versions"),
];

export const ORDERABLE_COLUMNS: readonly DataGridColumn<OrderableView>[] = [
  ...codedColumns<OrderableView>(),
  {
    id: "kind",
    header: "Kind",
    cell: (row) => labelOf(ORDERABLE_KINDS, row.kind),
    sortValue: (row) => row.kind,
    width: 240,
  },
  versionColumn("Versions"),
];
