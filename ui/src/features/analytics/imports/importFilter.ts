import { useCallback, useContext } from "react";
import { useLocation, useSearchParams } from "react-router";
import type { ImportOutcome } from "@/generated/ImportOutcome";
import type { ListImportAttemptsInput } from "@/generated/ListImportAttemptsInput";
import { type Choice, choiceOf } from "@/lib/choices";
import { isCalendarDate, LabTimeZoneContext, labDayStart } from "@/lib/format";
import { importStatus } from "../resultLabels";

/** The import list's filters, as kept in the address; an absent one shows every attempt. */
export interface ImportFilter {
  instrumentId?: string | undefined;
  outcome?: ImportOutcome | undefined;
  /** First and last received day, `YYYY-MM-DD` in the laboratory's time zone, both included. */
  from?: string | undefined;
  to?: string | undefined;
  /** A sample label, container label, accession number or file name, or part of one. */
  search?: string | undefined;
}

/** Each outcome the filter offers, worded as its badge; a later outcome is one more code. */
export const OUTCOME_FILTERS: readonly Choice<ImportOutcome>[] = (
  ["processed", "processed_with_quarantine", "quarantined"] as const
).map((id) => ({ id, label: importStatus(id).label }));

/** The address parameter of each filter. */
const PARAMS = {
  instrumentId: "instrument",
  outcome: "outcome",
  from: "from",
  to: "to",
  search: "q",
} as const satisfies Record<keyof ImportFilter, string>;

function filterFrom(params: URLSearchParams): ImportFilter {
  const text = (key: keyof ImportFilter) => params.get(PARAMS[key])?.trim() || undefined;
  const day = (key: "from" | "to") => {
    const value = text(key);
    return value && isCalendarDate(value) ? value : undefined;
  };
  return {
    instrumentId: text("instrumentId"),
    outcome: choiceOf(OUTCOME_FILTERS, text("outcome")) ?? undefined,
    from: day("from"),
    to: day("to"),
    search: text("search"),
  };
}

/** Whether any filter narrows the list. */
export const isFiltered = (filter: ImportFilter) => Object.values(filter).some(Boolean);

/**
 * The filters in the address (`?instrument=&outcome=&from=&to=&q=`), so a filtered list can
 * be shared, reloaded or reached again from an attempt's breadcrumb, and the call that applies
 * a whole new set. It replaces the history entry and keeps the navigation state.
 */
export function useImportFilter(): [ImportFilter, (next: ImportFilter) => void] {
  const [params, setParams] = useSearchParams();
  const { state } = useLocation();
  const apply = useCallback(
    (next: ImportFilter) => {
      const changed = new URLSearchParams();
      for (const [key, name] of Object.entries(PARAMS)) {
        const value = next[key as keyof ImportFilter];
        if (value) changed.set(name, value);
      }
      setParams(changed, { replace: true, state });
    },
    [setParams, state],
  );
  return [filterFrom(params), apply];
}

/**
 * The engine's filter for `filter`: received days become the instants they begin in the
 * laboratory's time zone, the end day included.
 */
export function useImportListInput(
  filter: ImportFilter,
): Omit<ListImportAttemptsInput, "limit" | "before" | "afterImportAttemptId"> {
  const timeZone = useContext(LabTimeZoneContext);
  return {
    instrumentId: filter.instrumentId ?? null,
    outcome: filter.outcome ?? null,
    receivedFrom: filter.from ? labDayStart(filter.from, timeZone) : null,
    receivedBefore: filter.to ? labDayStart(filter.to, timeZone, 1) : null,
    search: filter.search ?? null,
  };
}
