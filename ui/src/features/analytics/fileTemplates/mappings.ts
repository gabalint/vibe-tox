// What the person mapped from the names and words found in a sample (result files brief F7,
// F9, F10). Keyed by the exact text the file writes, which is never changed.
import type { PositionRole } from "@/generated/PositionRole";
import type { ValueWordMeaning } from "@/generated/ValueWordMeaning";

export interface Mappings {
  /** Analyte name as the file writes it → catalog analyte. */
  analytes: ReadonlyMap<string, string>;
  /** Long files: names counted as not results, such as internal standards. */
  notResults: ReadonlySet<string>;
  sampleTypes: ReadonlyMap<string, PositionRole>;
  valueWords: ReadonlyMap<string, ValueWordMeaning>;
}

export const NO_MAPPINGS: Mappings = {
  analytes: new Map(),
  notResults: new Set(),
  sampleTypes: new Map(),
  valueWords: new Map(),
};

/** `map` with `key` set to `value`, or without `key` when `value` is `null`. */
function changed<V>(map: ReadonlyMap<string, V>, key: string, value: V | null): Map<string, V> {
  const next = new Map(map);
  if (value === null) next.delete(key);
  else next.set(key, value);
  return next;
}

export function mapAnalyte(mappings: Mappings, name: string, analyteId: string | null): Mappings {
  return { ...mappings, analytes: changed(mappings.analytes, name, analyteId) };
}

/** A name is either an analyte's or not a result, never both (F10). */
export function markNotResult(mappings: Mappings, name: string, notResult: boolean): Mappings {
  const notResults = new Set(mappings.notResults);
  if (notResult) notResults.add(name);
  else notResults.delete(name);
  return {
    ...mappings,
    analytes: notResult ? changed(mappings.analytes, name, null) : mappings.analytes,
    notResults,
  };
}

export function mapSampleType(mappings: Mappings, word: string, role: PositionRole | null) {
  return { ...mappings, sampleTypes: changed(mappings.sampleTypes, word, role) };
}

export function mapValueWord(mappings: Mappings, word: string, meaning: ValueWordMeaning | null) {
  return { ...mappings, valueWords: changed(mappings.valueWords, word, meaning) };
}
