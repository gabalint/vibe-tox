import { describe, expect, it } from "vitest";
import type { LayoutCheckRow } from "@/generated/LayoutCheckRow";
import { mapAnalyte, mapSampleType, mapValueWord, markNotResult, NO_MAPPINGS } from "./mappings";
import {
  EMPTY_TEMPLATE,
  foundIn,
  fullLayout,
  readingLayout,
  type TemplateValues,
  testCodes,
  unitsOf,
} from "./templateValues";
import {
  FENTANYL,
  LONG_HEADER,
  LONG_LAYOUT,
  LONG_READING,
  MORPHINE,
  WIDE_HEADER,
} from "./testTemplates";

const LONG: TemplateValues["layout"] = {
  ...EMPTY_TEMPLATE.layout,
  encoding: "utf8",
  delimiter: "comma",
  headerLine: "2",
  headerRows: "one",
  shape: "long",
  analyteColumn: "2",
  valueColumn: "3",
  columns: { ...EMPTY_TEMPLATE.layout.columns, sampleLabel: "0", sampleType: "1", units: "none" },
  trimFields: "yes",
  endOfData: "end_of_file",
  declaredUnit: " ng/mL ",
};

describe("the layout the template page builds (result files brief §4, §13)", () => {
  it("is incomplete until every structural choice is made", () => {
    expect(readingLayout(LONG, null)).toBeNull();
    expect(readingLayout({ ...LONG, endOfData: null }, LONG_HEADER)).toBeNull();
    expect(readingLayout({ ...LONG, valueColumn: "9" }, LONG_HEADER)).toBeNull();
    expect(readingLayout({ ...LONG, shape: "wide" }, LONG_HEADER)).toBeNull();
  });

  it("reads the sample with the structure only, and finds the names and words to map", () => {
    const reading = readingLayout(LONG, LONG_HEADER);
    expect(reading).toMatchObject({
      shape: { kind: "long", analyteColumn: 2, valueColumn: 3 },
      columns: { sampleLabel: 0, sampleType: 1, units: null },
      sampleTypes: [],
      valueWords: [],
      notResults: [],
      declaredUnit: null,
    });
    expect(foundIn(LONG_READING)).toEqual({
      analyteNames: ["Morphine", "Morphine-D6", "Fentanyl"],
      typeWords: [
        { text: "Unknown", count: 3 },
        { text: "QC", count: 1 },
        { text: "Standard", count: 1 },
      ],
      valueTexts: [{ text: "N/F", count: 1 }],
      empty: 1,
    });
  });

  it("adds the units, the mapped words and names, and leaves unmapped ones out", () => {
    const reading = readingLayout(LONG, LONG_HEADER);
    if (reading === null) throw new Error("incomplete");
    const found = foundIn(LONG_READING);
    let mappings = mapAnalyte(NO_MAPPINGS, "Morphine", MORPHINE.id);
    mappings = mapAnalyte(mappings, "Morphine-D6", FENTANYL.id);
    mappings = markNotResult(mappings, "Morphine-D6", true);
    mappings = mapSampleType(mappings, "Standard", "calibrator");
    mappings = mapSampleType(mapSampleType(mappings, "QC", "control"), "QC", null);
    mappings = mapValueWord(mappings, "N/F", "below_loq");
    const layout = fullLayout(reading, unitsOf(LONG, LONG_HEADER), found, mappings);
    expect(layout).toEqual(LONG_LAYOUT);
    // A name is either an analyte's or not a result (F10).
    expect(testCodes(reading, found, mappings)).toEqual([
      { sourceTestCode: "Morphine", analyteId: MORPHINE.id },
    ]);
  });

  it("declares a unit only without a units column (F8)", () => {
    const withColumn = { ...LONG, columns: { ...LONG.columns, units: "4" } };
    expect(unitsOf(withColumn, LONG_HEADER)).toEqual({ column: 4, declaredUnit: null });
    expect(unitsOf({ ...LONG, declaredUnit: "  " }, LONG_HEADER)).toEqual({
      column: null,
      declaredUnit: null,
    });
  });

  it("names a wide file's analytes by its value columns, found in the sample or not", () => {
    const wide: TemplateValues["layout"] = {
      ...LONG,
      headerLine: "1",
      headerRows: "two",
      shape: "wide",
      valueField: "Final Conc.",
      analyteColumns: ["4", "6"],
      columns: { ...LONG.columns, sampleType: "1" },
    };
    const reading = readingLayout(wide, WIDE_HEADER);
    if (reading === null) throw new Error("incomplete");
    const nothingFound = { analyteNames: [], typeWords: [], valueTexts: [], empty: 0 };
    const mappings = markNotResult(
      mapAnalyte(NO_MAPPINGS, "Fentanyl Results", FENTANYL.id),
      "Fentanyl Results",
      false,
    );
    expect(testCodes(reading, nothingFound, mappings)).toEqual([
      { sourceTestCode: "Fentanyl Results", analyteId: FENTANYL.id },
    ]);
    expect(
      fullLayout(reading, unitsOf(wide, WIDE_HEADER), nothingFound, mappings).notResults,
    ).toEqual([]);
  });

  it("offers as value words only texts the reader does not read as a value (F7)", () => {
    const [base] = LONG_READING.rows;
    if (base === undefined) throw new Error("no row");
    const read = (valueText: string, reading: LayoutCheckRow["reading"]): LayoutCheckRow => ({
      ...base,
      valueText,
      reading,
    });
    const found = foundIn({
      ...LONG_READING,
      rows: [
        read("5", "quantity"),
        read("<10", "censored"),
        read("<0", "zero_or_negative"),
        read("N/A", "unmapped_word"),
        read("1,234.5", "malformed"),
      ],
    });
    expect(found.valueTexts.map(({ text }) => text)).toEqual(["<0", "N/A"]);
  });
});
