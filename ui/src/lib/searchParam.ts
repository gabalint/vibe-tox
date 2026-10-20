import { useCallback } from "react";
import { useLocation, useSearchParams } from "react-router";

/**
 * One URL parameter of the view, such as a filter (`?status=open`), so Back, reload and a
 * breadcrumb from a record page return to the same rows. Setting it replaces the history
 * entry, rather than adding one per change, and keeps the other parameters (`q`, `open`) and
 * the navigation state (a record page's breadcrumb and sequence). `null` or an empty string
 * removes it. The value is the URL's text: parse it with `choiceOf` before trusting it.
 */
export function useSearchParam(
  name: string,
): [value: string | null, setValue: (value: string | null) => void] {
  const [params, setParams] = useSearchParams();
  const { state } = useLocation();
  const setValue = useCallback(
    (value: string | null) =>
      setParams(
        (current) => {
          const next = new URLSearchParams(current);
          if (value === null || value === "") next.delete(name);
          else next.set(name, value);
          return next;
        },
        { replace: true, state },
      ),
    [name, setParams, state],
  );
  return [params.get(name), setValue];
}
