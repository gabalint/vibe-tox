import { useCallback } from "react";
import { useSearchParam } from "./searchParam";

/**
 * A directory's find text, kept in `?q=` so Back, reload and the breadcrumb from a record
 * page return to the same rows. Changing it replaces the history entry rather than adding
 * one per pause in typing, and keeps the other parameters and the navigation state.
 */
export function useFindParam(): [query: string, setQuery: (text: string) => void] {
  const [query, setValue] = useSearchParam("q");
  const setQuery = useCallback(
    (text: string) => setValue(text.trim() === "" ? null : text),
    [setValue],
  );
  return [query ?? "", setQuery];
}
