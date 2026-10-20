// Labels of the file template's choices and of a check's outcomes, in laboratory terms
// (result files brief §4, §5, §7). The one place these engine codes are worded.
import type { EndOfData } from "@/generated/EndOfData";
import type { FileDelimiter } from "@/generated/FileDelimiter";
import type { FileEncoding } from "@/generated/FileEncoding";
import type { FileExtractFailure } from "@/generated/FileExtractFailure";
import type { FileShape } from "@/generated/FileShape";
import type { ReadingKind } from "@/generated/ReadingKind";
import type { ValueWordMeaning } from "@/generated/ValueWordMeaning";
import type { Choice } from "@/lib/choices";

export const ENCODINGS: readonly Choice<FileEncoding>[] = [
  { id: "utf8", label: "UTF-8" },
  { id: "windows1252", label: "Windows-1252" },
];

export const DELIMITERS: readonly Choice<FileDelimiter>[] = [
  { id: "comma", label: "Comma" },
  { id: "tab", label: "Tab" },
  { id: "semicolon", label: "Semicolon" },
];

export const HEADER_ROWS: readonly Choice<"one" | "two">[] = [
  { id: "one", label: "One line" },
  { id: "two", label: "Two lines: group labels, then field names" },
];

export const SHAPES: readonly Choice<FileShape["kind"]>[] = [
  { id: "long", label: "Long: one row per sample and analyte" },
  { id: "wide", label: "Wide: one row per sample, a column or column group per analyte" },
];

/** A file source's shape in a few words, such as "Wide, 3 analyte columns". */
export function shapeText(shape: FileShape): string {
  if (shape.kind === "long") return "Long, one row per sample and analyte";
  const count = shape.valueColumns.length;
  return `Wide, ${count} analyte ${count === 1 ? "column" : "columns"}`;
}

export const TRIM_FIELDS: readonly Choice<"yes" | "no">[] = [
  { id: "yes", label: "Remove spaces around each value" },
  { id: "no", label: "Keep values exactly as written" },
];

export const END_OF_DATA: readonly Choice<EndOfData>[] = [
  { id: "end_of_file", label: "At the end of the file" },
  { id: "first_blank_line", label: "At the first blank line (the export has a footer)" },
];

/** The choice of an optional column that the file does not have. */
export const NOT_IN_FILE = "none";

export const VALUE_MEANINGS: readonly Choice<ValueWordMeaning>[] = [
  { id: "below_loq", label: "Below the limit of quantitation" },
  { id: "below_lod", label: "Below the limit of detection" },
  { id: "not_performed", label: "Not performed" },
];

/** The choice of a word left unmapped: its rows are quarantined. */
export const UNMAPPED = "unmapped";

const READINGS: Record<ReadingKind, string> = {
  quantity: "Number",
  censored: "Censored value",
  word: "Mapped word",
  unmapped_word: "Unmapped word, quarantined",
  empty: "Empty, quarantined",
  zero_or_negative: "Zero or negative, quarantined",
  malformed: "Not a number, quarantined",
};

export function readingText(reading: ReadingKind, meaning: ValueWordMeaning | null): string {
  if (reading === "word" && meaning !== null) {
    const label = VALUE_MEANINGS.find((choice) => choice.id === meaning)?.label ?? meaning;
    return `Mapped word: ${label.toLowerCase()}`;
  }
  return READINGS[reading];
}

const FAILURES: Record<FileExtractFailure, string> = {
  undecodable_text:
    "The file is not text in the template's encoding. Save the export as UTF-8, or choose Windows-1252 if the software writes it.",
  layout_mismatch:
    "No line among the first 50 matches the template's header. A column was added, removed, renamed or moved, or a group label changed.",
  limits_exceeded:
    "The file has more rows, columns or characters in one value than the LIS reads: 50,000 rows, 512 columns, 4 KiB per value.",
};

/** Why a whole file gives no rows, in plain words. */
export function failureText(failure: FileExtractFailure): string {
  return FAILURES[failure];
}
