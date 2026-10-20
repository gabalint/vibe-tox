import type { UseQueryResult } from "@tanstack/react-query";
import { useMemo } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ChoiceInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { CommandError } from "@/generated/CommandError";
import type { PreviewLine } from "@/generated/PreviewLine";
import { ChoiceField } from "./ChoiceField";
import { HEADER_ROWS } from "./templateLabels";
import type { TemplateValues } from "./templateValues";

/** The cells shown of each line in the header-line choice. */
const LINE_CELLS = 4;

function previewColumns(lines: readonly PreviewLine[]): DataGridColumn<PreviewLine>[] {
  const width = Math.max(0, ...lines.map((line) => line.cells.length));
  return [
    { id: "line", header: "Line", cell: (row) => row.line, align: "end", width: 72 },
    ...Array.from({ length: width }, (_, index) => ({
      id: `cell-${index}`,
      header: String(index + 1),
      cell: (row: PreviewLine) => row.cells[index] ?? "",
    })),
  ];
}

const lineOption = (line: PreviewLine) => ({
  id: String(line.line),
  label: `Line ${line.line}: ${line.cells
    .filter((cell) => cell !== "")
    .slice(0, LINE_CELLS)
    .join(", ")}`,
});

interface SamplePreviewProps {
  preview: UseQueryResult<PreviewLine[], CommandError>;
  header: string[][] | null;
}

/**
 * The sample's first 50 lines as cells with their line numbers (brief §7, step 1). Choosing a
 * line, in the field or by a click or Enter on its row, makes it the header.
 */
export function SamplePreview({ preview, header }: SamplePreviewProps) {
  return (
    <QueryContent
      query={preview}
      what="sample lines"
      loadingRows={6}
      empty={{
        when: (lines) => lines.length === 0,
        title: "The sample is empty",
        description: "Open an export that holds at least its header line.",
      }}
    >
      {(lines) => <HeaderChoice lines={lines} header={header} />}
    </QueryContent>
  );
}

function HeaderChoice({ lines, header }: { lines: PreviewLine[]; header: string[][] | null }) {
  const { setValue } = useFormContext<TemplateValues>();
  const headerLine = useWatch<TemplateValues, "layout.headerLine">({ name: "layout.headerLine" });
  const columns = useMemo(() => previewColumns(lines), [lines]);
  const [first, second] = header ?? [];
  const unequal = first !== undefined && second !== undefined && first.length !== second.length;
  return (
    <>
      <DataGrid
        label="First lines of the sample"
        columns={columns}
        rows={lines}
        getRowId={(line) => String(line.line)}
        onRowAction={(line) =>
          setValue("layout.headerLine", String(line.line), {
            shouldDirty: true,
            shouldValidate: true,
          })
        }
        openRowId={headerLine}
      />
      <ChoiceField
        name="layout.headerLine"
        label="Header line"
        options={lines.map(lineOption)}
        isRequired
      />
      <ChoiceInput name="layout.headerRows" label="Header rows" options={HEADER_ROWS} isRequired />
      {unequal && (
        <Alert tone="warning" title="The two header lines have different numbers of cells">
          Both lines need the same columns: {first.length} and {second.length} were found. Check the
          delimiter and the header line.
        </Alert>
      )}
    </>
  );
}
