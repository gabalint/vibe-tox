import { useMemo } from "react";
import { Alert } from "@/components/Alert";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { EmptyState } from "@/components/EmptyState";
import { Facts } from "@/components/Facts";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import type { LayoutCheckCounts } from "@/generated/LayoutCheckCounts";
import type { LayoutCheckRow } from "@/generated/LayoutCheckRow";
import { labelOf } from "@/lib/choices";
import { POSITION_ROLES } from "../labels";
import { failureText, readingText } from "./templateLabels";

/** Each count of the check, quarantined ones said as such (brief §5). */
const COUNTS: readonly [keyof LayoutCheckCounts, string][] = [
  ["rows", "Rows read"],
  ["quantities", "Numbers"],
  ["censored", "Censored values"],
  ["words", "Mapped words"],
  ["unmappedWords", "Unmapped words, quarantined"],
  ["empty", "Empty values, quarantined"],
  ["zeroOrNegative", "Zero or negative values, quarantined"],
  ["malformedValues", "Values that are not numbers, quarantined"],
  ["unmappedAnalytes", "Rows of unmapped analytes, quarantined"],
  ["notResults", "Rows that are not results, counted only"],
  ["untyped", "Rows without a mapped sample type"],
  ["malformedRows", "Records with a different number of fields, quarantined"],
];

/** Text the file left empty, said rather than shown blank. */
const EMPTY = "Empty";

/**
 * A wide file's own header text for each value column (its header cell, or the group label of
 * a two-row header), beside the mapped analyte, so the approver compares the two.
 */
const HEADER_TEXT: DataGridColumn<LayoutCheckRow> = {
  id: "header",
  header: "File's header text",
  cell: (row) => row.headerText ?? EMPTY,
  wrap: true,
};

function checkColumns(
  analyteName: (id: string) => string,
  wide: boolean,
): DataGridColumn<LayoutCheckRow>[] {
  return [
    { id: "reference", header: "Reference", cell: (row) => row.recordPath, width: 120 },
    { id: "label", header: "Sample label", cell: (row) => row.sampleLabel ?? EMPTY },
    { id: "analyte", header: "Analyte as written", cell: (row) => row.analyteName ?? EMPTY },
    ...(wide ? [HEADER_TEXT] : []),
    {
      id: "mapped",
      header: "Catalog analyte",
      cell: (row) => {
        if (row.notResult) return "Not a result";
        return row.analyteId === null ? "Not mapped, quarantined" : analyteName(row.analyteId);
      },
      wrap: true,
    },
    { id: "value", header: "Value as written", cell: (row) => row.valueText ?? EMPTY },
    {
      id: "reading",
      header: "Value reads as",
      cell: (row) => readingText(row.reading, row.wordMeaning),
      wrap: true,
    },
    {
      id: "type",
      header: "Sample type",
      cell: (row) => {
        if (row.typeWord === null) return "None";
        const role = row.role === null ? "not mapped" : labelOf(POSITION_ROLES, row.role);
        return `${row.typeWord}: ${role}`;
      },
      wrap: true,
    },
  ];
}

interface LayoutCheckResultProps {
  check: LayoutCheck;
  /** The catalog analyte's name and code. */
  analyteName: (id: string) => string;
}

/**
 * What a template makes of a file, row by row, with the counts (brief §7, step 6): or why the
 * whole file gives no rows. The rows hold sample labels and values from the file; they are
 * shown here only and never stored or logged.
 */
export function LayoutCheckResult({ check, analyteName }: LayoutCheckResultProps) {
  // Only a wide file names its analytes by header text; a long file's rows carry the name.
  const wide = check.rows.some((row) => row.headerText !== null);
  const columns = useMemo(() => checkColumns(analyteName, wide), [analyteName, wide]);
  if (check.failure !== null) {
    return (
      <Alert tone="danger" title="No row can be read from this file">
        {failureText(check.failure)}
      </Alert>
    );
  }
  return (
    <>
      <Facts items={COUNTS.map(([key, label]) => ({ label, value: String(check.counts[key]) }))} />
      {check.malformedRows.length > 0 && (
        <p>Records with a different number of fields: {check.malformedRows.join(", ")}.</p>
      )}
      {check.rows.length === 0 ? (
        <EmptyState title="No result rows" description="The file has no rows after its header." />
      ) : (
        <DataGrid
          label="Rows read from the file"
          columns={columns}
          rows={check.rows}
          getRowId={(row) => row.recordPath}
        />
      )}
    </>
  );
}
