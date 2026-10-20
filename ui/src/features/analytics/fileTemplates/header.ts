// The header of a sample export as the template page reads it (result files brief §7, F3–F5):
// the signature rows the person chose, the columns by position, and a two-row header's
// groups. Presentation only: the engine validates the layout and reads the file.
import type { PreviewLine } from "@/generated/PreviewLine";
import type { ValueColumn } from "@/generated/ValueColumn";

/**
 * The signature: the chosen header line and, for a two-row header, the line after it.
 * `null` when the line is not among the previewed ones, or a second row is missing.
 */
export function headerRows(
  lines: readonly PreviewLine[],
  headerLine: number,
  twoRows: boolean,
): string[][] | null {
  const index = lines.findIndex((line) => line.line === headerLine);
  const first = lines[index];
  if (index < 0 || first === undefined) return null;
  if (!twoRows) return [first.cells];
  const second = lines[index + 1];
  return second === undefined ? null : [first.cells, second.cells];
}

/** The cells of the first line after the header, as an example of each column's values. */
export function exampleCells(
  lines: readonly PreviewLine[],
  headerLine: number,
  rowCount: number,
): readonly string[] {
  const index = lines.findIndex((line) => line.line === headerLine);
  return lines[index + rowCount]?.cells ?? [];
}

/** One column of the header, named for the person choosing it. */
export interface ColumnOption {
  /** The 0-based index, as text: the drop-downs' value. */
  id: string;
  /** Position from 1 and the header text, such as `3: Final Conc.`. */
  label: string;
  /** The first data line's value, when there is one. */
  description?: string;
}

/**
 * A cell as a layout reads it: without surrounding spaces when it trims (F6). Mirrors the
 * engine's rule, which removes spaces only, so names built here match what it accepts.
 */
export function field(text: string, trim: boolean): string {
  return trim ? text.replace(/^ +| +$/g, "") : text;
}

/**
 * A two-row header's group label for each column: the nearest non-empty label at or before
 * it, as the layout reads cells. Labels are written only at a group's first column.
 */
function groupLabels(header: readonly string[][], trim: boolean): string[] {
  const groups = header[0] ?? [];
  let current = "";
  return (header[1] ?? groups).map((_, index) => {
    const label = field(groups[index] ?? "", trim);
    if (label !== "") current = label;
    return current;
  });
}

/** Each column of the last header row, by position, with an example value. */
export function columnOptions(
  header: readonly string[][],
  examples: readonly string[],
  trim: boolean,
): ColumnOption[] {
  const fields = header[header.length - 1] ?? [];
  const groups = header.length === 2 ? groupLabels(header, trim) : [];
  return fields.map((cell, index) => {
    const name = [groups[index], field(cell, trim)].filter(
      (part) => part !== undefined && part !== "",
    );
    const example = examples[index] ?? "";
    return {
      id: String(index),
      label: `${index + 1}: ${name.length > 0 ? name.join(" / ") : "(no header text)"}`,
      ...(example !== "" && { description: `for example ${example}` }),
    };
  });
}

/** The distinct field names of a two-row header's second row, in order. */
export function fieldNames(header: readonly string[][], trim: boolean): string[] {
  const names = (header[1] ?? []).map((name) => field(name, trim));
  return [...new Set(names.filter((name) => name !== ""))];
}

/** One group of a two-row header and its column holding `field`, if it has one. */
export interface HeaderGroup {
  label: string;
  /** The value column, or `null` when the group has no such field and gives no rows. */
  column: number | null;
}

/**
 * The labelled groups of a two-row header, each with its first column whose field is
 * `valueField` (F4). A group without that field, such as a qualifier group, gives no rows.
 */
export function headerGroups(
  header: readonly string[][],
  valueField: string,
  trim: boolean,
): HeaderGroup[] {
  const labels = groupLabels(header, trim);
  const fields = header[1] ?? [];
  const groups: HeaderGroup[] = [];
  labels.forEach((label, index) => {
    if (label === "") return;
    let group = groups.find((candidate) => candidate.label === label);
    if (group === undefined) {
      group = { label, column: null };
      groups.push(group);
    }
    if (group.column === null && field(fields[index] ?? "", trim) === valueField) {
      group.column = index;
    }
  });
  return groups;
}

/**
 * The value columns of a wide file, each named by the file's own text as the layout reads
 * it (the engine refuses any other name): for a two-row header the kept groups' value
 * columns, named by the group label; for one row the ticked columns, by their header cell.
 */
export function valueColumns(
  header: readonly string[][],
  columns: readonly string[],
  trim: boolean,
): ValueColumn[] {
  const labels =
    header.length === 2
      ? groupLabels(header, trim)
      : (header[0] ?? []).map((cell) => field(cell, trim));
  return columns
    .map(Number)
    .sort((a, b) => a - b)
    .map((column) => ({ column, analyteName: labels[column] ?? "" }));
}
