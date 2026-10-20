// The template page's values and the layout they describe (result files brief §4, §7, §13).
// Presentation only: the engine validates the layout fully when it checks and saves it.
import type { EndOfData } from "@/generated/EndOfData";
import type { FileColumns } from "@/generated/FileColumns";
import type { FileDelimiter } from "@/generated/FileDelimiter";
import type { FileEncoding } from "@/generated/FileEncoding";
import type { FileLayout } from "@/generated/FileLayout";
import type { FileShape } from "@/generated/FileShape";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import type { RegisterFileSourceInput } from "@/generated/RegisterFileSourceInput";
import type { SourceTestCodeInput } from "@/generated/SourceTestCodeInput";
import { choiceOf, optionalText } from "@/lib/choices";
import { valueColumns } from "./header";
import type { Mappings } from "./mappings";
import { DELIMITERS, ENCODINGS, END_OF_DATA, NOT_IN_FILE } from "./templateLabels";

/** The optional column roles, besides the required sample label (F5). */
const OPTIONAL_COLUMNS = [
  "units",
  "sampleType",
  "flags",
  "operator",
  "acquiredAt",
  "dilution",
] as const;
type OptionalColumn = (typeof OPTIONAL_COLUMNS)[number];

/**
 * The fields bound to the form. Choices hold engine codes, or column indexes and line
 * numbers as text; `null` until chosen. Names follow the engine's input where they can, so
 * its field errors show on the matching field.
 */
export interface TemplateValues {
  softwareName: string;
  softwareVersion: string;
  reportsFinalConcentration: string | null;
  layout: {
    encoding: string | null;
    delimiter: string | null;
    headerLine: string | null;
    headerRows: string | null;
    shape: string | null;
    analyteColumn: string | null;
    valueColumn: string | null;
    valueField: string | null;
    /** Wide: the ticked columns, or the kept groups' value columns of a two-row header. */
    analyteColumns: string[];
    columns: Record<"sampleLabel" | OptionalColumn, string | null>;
    trimFields: string | null;
    endOfData: string | null;
    declaredUnit: string;
  };
}

export const EMPTY_TEMPLATE: TemplateValues = {
  softwareName: "",
  softwareVersion: "",
  reportsFinalConcentration: null,
  layout: {
    encoding: null,
    delimiter: null,
    headerLine: null,
    headerRows: null,
    shape: null,
    analyteColumn: null,
    valueColumn: null,
    valueField: null,
    analyteColumns: [],
    columns: {
      sampleLabel: null,
      units: null,
      sampleType: null,
      flags: null,
      operator: null,
      acquiredAt: null,
      dilution: null,
    },
    trimFields: null,
    endOfData: null,
    declaredUnit: "",
  },
};

/** Engine paths that the form names differently. */
export const TEMPLATE_FIELD_PATHS = {
  "layout.shape": "layout",
  "layout.header": "layout.headerLine",
} as const;

/** Whether the person chose to remove spaces around values (F6). */
export function trims(layout: Pick<TemplateValues["layout"], "trimFields">): boolean {
  return layout.trimFields === "yes";
}

/** A chosen column index, or `null` when none is chosen or it is not in the header. */
function column(value: string | null, width: number): number | null {
  if (value === null || value === NOT_IN_FILE) return null;
  const index = Number.parseInt(value, 10);
  return Number.isInteger(index) && index >= 0 && index < width ? index : null;
}

function shapeOf(layout: TemplateValues["layout"], header: string[][]): FileShape | null {
  const width = header[header.length - 1]?.length ?? 0;
  if (layout.shape === "long") {
    const analyteColumn = column(layout.analyteColumn, width);
    const valueColumn = column(layout.valueColumn, width);
    if (analyteColumn === null || valueColumn === null) return null;
    return { kind: "long", analyteColumn, valueColumn };
  }
  if (layout.shape === "wide" && layout.analyteColumns.length > 0) {
    return {
      kind: "wide",
      valueColumns: valueColumns(header, layout.analyteColumns, trims(layout)),
    };
  }
  return null;
}

function columnsOf(layout: TemplateValues["layout"], width: number): FileColumns | null {
  const sampleLabel = column(layout.columns.sampleLabel, width);
  if (sampleLabel === null) return null;
  const at = (role: OptionalColumn) => column(layout.columns[role], width);
  return {
    sampleLabel,
    // Added by `withUnits`: a unit changes nothing that reading the sample finds.
    units: null,
    sampleType: at("sampleType"),
    flags: at("flags"),
    operator: at("operator"),
    acquiredAt: at("acquiredAt"),
    dilution: at("dilution"),
  };
}

/**
 * The structure the person chose, without units or any mapped name or word: the sample is
 * read with it to find the names and words to map. `null` until every structural choice is
 * made.
 */
export function readingLayout(
  layout: TemplateValues["layout"],
  header: string[][] | null,
): FileLayout | null {
  const encoding: FileEncoding | null = choiceOf(ENCODINGS, layout.encoding);
  const delimiter: FileDelimiter | null = choiceOf(DELIMITERS, layout.delimiter);
  const endOfData: EndOfData | null = choiceOf(END_OF_DATA, layout.endOfData);
  if (header === null || encoding === null || delimiter === null || endOfData === null) {
    return null;
  }
  const shape = shapeOf(layout, header);
  const columns = columnsOf(layout, header[header.length - 1]?.length ?? 0);
  if (shape === null || columns === null || layout.trimFields === null) return null;
  return {
    encoding,
    delimiter,
    trimFields: layout.trimFields === "yes",
    header,
    shape,
    columns,
    endOfData,
    sampleTypes: [],
    valueWords: [],
    notResults: [],
    declaredUnit: null,
  };
}

/** Where values' units come from (F8): a units column, else a declared unit, or neither. */
export interface Units {
  column: number | null;
  declaredUnit: string | null;
}

export function unitsOf(layout: TemplateValues["layout"], header: string[][]): Units {
  const unitsColumn = column(layout.columns.units, header[header.length - 1]?.length ?? 0);
  return {
    column: unitsColumn,
    // F8: a declared unit only without a units column.
    declaredUnit: unitsColumn === null ? optionalText(layout.declaredUnit) : null,
  };
}

/** A distinct text found in the sample and how many rows carry it. */
export interface Counted {
  text: string;
  count: number;
}

/** What reading the sample found to map (brief §7, steps 5 and 6). */
export interface Found {
  /** Long files: the distinct analyte names, in file order. */
  analyteNames: readonly string[];
  /** The distinct non-empty sample-type words. */
  typeWords: readonly Counted[];
  /** The distinct value texts that are not positive numbers, which a word may map (F7). */
  valueTexts: readonly Counted[];
  /** Rows whose value is empty: always quarantined, never mapped. */
  empty: number;
}

function counted(texts: readonly (string | null)[]): Counted[] {
  const counts = new Map<string, number>();
  for (const text of texts) {
    if (text !== null && text !== "") counts.set(text, (counts.get(text) ?? 0) + 1);
  }
  return [...counts].map(([text, count]) => ({ text, count }));
}

/** The names and words in a reading of the sample with no mapping. */
export function foundIn(check: LayoutCheck): Found {
  const mappable = check.rows.filter(
    (row) => row.reading === "unmapped_word" || row.reading === "zero_or_negative",
  );
  return {
    analyteNames: counted(check.rows.map((row) => row.analyteName)).map(({ text }) => text),
    typeWords: counted(check.rows.map((row) => row.typeWord)),
    valueTexts: counted(mappable.map((row) => row.valueText)),
    empty: check.counts.empty,
  };
}

/** The analyte names to map: a wide file's value columns, or a long file's names found. */
export function analyteNames(reading: FileLayout, found: Found): readonly string[] {
  return reading.shape.kind === "wide"
    ? reading.shape.valueColumns.map(({ analyteName }) => analyteName)
    : found.analyteNames;
}

/** The mapped analyte names, exactly as the file writes them (REQ-ANA-003). */
export function testCodes(
  reading: FileLayout,
  found: Found,
  mappings: Mappings,
): SourceTestCodeInput[] {
  return analyteNames(reading, found).flatMap((name) => {
    const analyteId = mappings.analytes.get(name);
    return analyteId === undefined || isNotResult(reading, mappings, name)
      ? []
      : [{ sourceTestCode: name, analyteId }];
  });
}

function isNotResult(reading: FileLayout, mappings: Mappings, name: string): boolean {
  return reading.shape.kind === "long" && mappings.notResults.has(name);
}

/**
 * The whole layout: the structure with its units, and the words and names mapped for what
 * was found.
 */
export function fullLayout(
  reading: FileLayout,
  units: Units,
  found: Found,
  mappings: Mappings,
): FileLayout {
  return {
    ...reading,
    columns: { ...reading.columns, units: units.column },
    declaredUnit: units.declaredUnit,
    sampleTypes:
      reading.columns.sampleType === null
        ? []
        : found.typeWords.flatMap(({ text }) => {
            const role = mappings.sampleTypes.get(text);
            return role === undefined ? [] : [{ word: text, role }];
          }),
    valueWords: found.valueTexts.flatMap(({ text }) => {
      const meaning = mappings.valueWords.get(text);
      return meaning === undefined ? [] : [{ word: text, meaning }];
    }),
    notResults: analyteNames(reading, found).filter((name) => isNotResult(reading, mappings, name)),
  };
}

/** The draft registration: the software as declared, the layout and its test codes (F2). */
export function toRegisterInput(
  instrumentId: string,
  values: TemplateValues,
  layout: FileLayout,
  codes: SourceTestCodeInput[],
): RegisterFileSourceInput {
  return {
    instrumentId,
    softwareName: values.softwareName.trim(),
    softwareVersion: values.softwareVersion.trim(),
    reportsFinalConcentration: values.reportsFinalConcentration === "yes",
    layout,
    testCodes: codes,
  };
}
