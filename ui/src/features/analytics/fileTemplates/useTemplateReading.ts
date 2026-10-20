import { useMemo } from "react";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import type { LayoutSampleOpened } from "@/generated/LayoutSampleOpened";
import type { PreviewLine } from "@/generated/PreviewLine";
import { choiceOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { CHECK_FILE_LAYOUT, PREVIEW_LAYOUT_SAMPLE } from "../contract";
import { columnOptions, exampleCells, headerRows } from "./header";
import { DELIMITERS, ENCODINGS } from "./templateLabels";
import { foundIn, readingLayout, type TemplateValues, trims } from "./templateValues";

/**
 * What the open sample shows under the choices made so far: its first lines as cells once
 * the encoding and delimiter are chosen, the header and its columns once the header line is,
 * and the names and words to map once the structure is complete. Each read runs only when
 * its choices change; none writes anything.
 */
export function useTemplateReading(
  sample: LayoutSampleOpened | null,
  layout: TemplateValues["layout"],
) {
  const encoding = choiceOf(ENCODINGS, layout.encoding);
  const delimiter = choiceOf(DELIMITERS, layout.delimiter);
  const sampleHandle = sample?.sampleHandle ?? null;
  const canPreview = sampleHandle !== null && encoding !== null && delimiter !== null;
  const preview = useCommandQuery<PreviewLine[]>(
    PREVIEW_LAYOUT_SAMPLE,
    { input: { sampleHandle, encoding, delimiter } },
    { enabled: canPreview },
  );
  const lines = canPreview ? preview.data : undefined;
  const headerLine = layout.headerLine === null ? null : Number.parseInt(layout.headerLine, 10);
  const header =
    lines === undefined || headerLine === null || layout.headerRows === null
      ? null
      : headerRows(lines, headerLine, layout.headerRows === "two");
  const columns =
    header === null || lines === undefined || headerLine === null
      ? []
      : columnOptions(header, exampleCells(lines, headerLine, header.length), trims(layout));
  const reading = sampleHandle === null ? null : readingLayout(layout, header);
  const readingQuery = useCommandQuery<LayoutCheck>(
    CHECK_FILE_LAYOUT,
    { input: { sampleHandle, layout: reading, testCodes: [] } },
    { enabled: reading !== null },
  );
  const check = reading === null ? undefined : readingQuery.data;
  const found = useMemo(
    () => (check === undefined || check.failure !== null ? null : foundIn(check)),
    [check],
  );
  return {
    preview: canPreview ? preview : null,
    header,
    columns,
    reading,
    readingQuery,
    found,
  };
}
