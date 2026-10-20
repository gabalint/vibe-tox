import { describe, expect, it } from "vitest";
import {
  columnOptions,
  exampleCells,
  field,
  fieldNames,
  headerGroups,
  headerRows,
  valueColumns,
} from "./header";
import { LONG_HEADER, LONG_LINES, WIDE_HEADER, WIDE_LINES } from "./testTemplates";

describe("the header of a sample export (result files brief F3–F5)", () => {
  it("takes the chosen line, and the next one for a two-row header", () => {
    expect(headerRows(LONG_LINES, 2, false)).toEqual(LONG_HEADER);
    expect(headerRows(WIDE_LINES, 1, true)).toEqual(WIDE_HEADER);
    expect(headerRows(LONG_LINES, 99, false)).toBeNull();
    expect(headerRows(LONG_LINES, 7, true)).toBeNull();
  });

  it("names each column by position and header text, with the first data line's value", () => {
    const options = columnOptions(LONG_HEADER, exampleCells(LONG_LINES, 2, 1), false);
    expect(options[0]).toEqual({
      id: "0",
      label: "1: Sample Name",
      description: "for example C00000123",
    });
    expect(options[3]).toEqual({
      id: "3",
      label: "4: Final Conc",
      description: "for example 152.4",
    });
  });

  it("names a two-row header's columns by group and field, and says when a cell is empty", () => {
    const options = columnOptions(WIDE_HEADER, exampleCells(WIDE_LINES, 1, 2), false);
    expect(options.map(({ label }) => label)).toEqual([
      "1: Sample / Name",
      "2: Sample / Type",
      "3: Sample / Dil.",
      "4: Morphine Results / RT",
      "5: Morphine Results / Final Conc.",
      "6: Fentanyl Results / RT",
      "7: Fentanyl Results / Final Conc.",
      "8: Qualifier (338.2 -> 105.1) Results / Ratio",
    ]);
    expect(options[5]?.description).toBeUndefined();
    expect(columnOptions([["", "Name"]], [], false)[0]?.label).toBe("1: (no header text)");
  });

  it("finds each group's value field; a group without it gives no rows (F4)", () => {
    expect(fieldNames(WIDE_HEADER, false)).toEqual([
      "Name",
      "Type",
      "Dil.",
      "RT",
      "Final Conc.",
      "Ratio",
    ]);
    expect(headerGroups(WIDE_HEADER, "Final Conc.", false)).toEqual([
      { label: "Sample", column: null },
      { label: "Morphine Results", column: 4 },
      { label: "Fentanyl Results", column: 6 },
      { label: "Qualifier (338.2 -> 105.1) Results", column: null },
    ]);
  });

  it("names wide value columns by group label, or by header text for one row", () => {
    expect(valueColumns(WIDE_HEADER, ["6", "4"], false)).toEqual([
      { column: 4, analyteName: "Morphine Results" },
      { column: 6, analyteName: "Fentanyl Results" },
    ]);
    expect(valueColumns([["Sample", "Morphine", "Fentanyl"]], ["2"], false)).toEqual([
      { column: 2, analyteName: "Fentanyl" },
    ]);
  });

  it("names analytes by the header text as the layout reads it, trimmed when it trims (F6)", () => {
    const spaced = [
      ["Sample", "", " Morphine Results ", "", "  ", ""],
      ["Name", "Type", "RT", " Final Conc.", "RT", "Final Conc. "],
    ];
    // The blank label after trimming carries the group before it on, as the engine does.
    expect(fieldNames(spaced, true)).toEqual(["Name", "Type", "RT", "Final Conc."]);
    expect(headerGroups(spaced, "Final Conc.", true)).toEqual([
      { label: "Sample", column: null },
      { label: "Morphine Results", column: 3 },
    ]);
    expect(valueColumns(spaced, ["3"], true)).toEqual([
      { column: 3, analyteName: "Morphine Results" },
    ]);
    expect(valueColumns(spaced, ["3"], false)).toEqual([
      { column: 3, analyteName: " Morphine Results " },
    ]);
    expect(valueColumns([["Sample", " Fentanyl "]], ["1"], true)).toEqual([
      { column: 1, analyteName: "Fentanyl" },
    ]);
    // Only spaces are removed, as the engine removes them.
    expect(field("\tN/F ", true)).toBe("\tN/F");
  });
});
