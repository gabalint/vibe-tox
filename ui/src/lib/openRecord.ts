import { useCallback, useState } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

/** The URL parameter holding the record open in a side panel. */
const OPEN_PARAM = "open";

/**
 * Navigation state key set on the entry that opening a record pushed. Its value is the
 * address without `?open=` at that moment, so closing goes back only when nothing else
 * (find text, filters) changed since.
 */
const PUSHED_FROM = "openRecordPushedFrom";

/** The navigation state as an object, so other keys (such as a record page's) are kept. */
function stateObject(state: unknown): object {
  return typeof state === "object" && state !== null ? state : {};
}

function pushedFrom(state: unknown): string | null {
  const object = stateObject(state);
  if (!(PUSHED_FROM in object)) return null;
  const from: unknown = object[PUSHED_FROM];
  return typeof from === "string" ? from : null;
}

/** The state without the pushed marker; null when nothing else is left. */
function withoutPushed(state: unknown): object | null {
  const rest = Object.entries(stateObject(state)).filter(([key]) => key !== PUSHED_FROM);
  return rest.length === 0 ? null : Object.fromEntries(rest);
}

/** The address without `?open=`. */
function addressWithout(pathname: string, params: URLSearchParams): string {
  const rest = new URLSearchParams(params);
  rest.delete(OPEN_PARAM);
  return `${pathname}?${rest.toString()}`;
}

function sameOrder(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((id, index) => id === b[index]);
}

export interface OpenRecord {
  /** The open record's ID, from `?open=`; null when no record is open. */
  openId: string | null;
  /**
   * Opens a record, keeping the other parameters such as `q` and filters and the page's
   * navigation state. Opening from a closed panel adds a history entry, so Back closes it.
   * `order` is the list's row IDs in display order, which Previous and Next follow.
   */
  open: (id: string, order?: readonly string[]) => void;
  /**
   * Follows the list's current display order: pass it to the list's `DataGrid` as
   * `onOrderChange`, so Previous and Next follow a sort or find made with the panel open.
   * Stable between renders.
   */
  followOrder: (order: readonly string[]) => void;
  /**
   * Closes the record: goes back when opening added the entry and only `open` changed since;
   * otherwise removes `?open=` in place, keeping the find text and filters typed meanwhile.
   */
  close: () => void;
  /** Opens the record before the open one in the list's order; undefined at the start. */
  previous: (() => void) | undefined;
  /** Opens the record after the open one in the list's order; undefined at the end. */
  next: (() => void) | undefined;
}

/**
 * The record open in a side panel, held in the URL (`?open=<id>`). It only changes the URL
 * on an explicit open, Previous, Next or close; the screen loads the record for `openId`,
 * so nothing is fetched on hover, focus or arrow keys, and nothing is prefetched. Previous
 * and Next follow the list's display order, which its `DataGrid` reports (`followOrder`);
 * a record its list does not show has no neighbours, so both are undefined. Navigation state
 * already on the entry, such as a record page's breadcrumb and sequence, is kept.
 */
export function useOpenRecord(): OpenRecord {
  const [params, setParams] = useSearchParams();
  const { pathname, state } = useLocation();
  const navigate = useNavigate();
  const [order, setOrder] = useState<readonly string[]>([]);
  const openId = params.get(OPEN_PARAM);
  const followOrder = useCallback(
    (next: readonly string[]) => setOrder((current) => (sameOrder(current, next) ? current : next)),
    [],
  );

  function show(id: string | null, options: { replace: boolean; state: unknown }) {
    setParams((current) => {
      const next = new URLSearchParams(current);
      if (id === null) next.delete(OPEN_PARAM);
      else next.set(OPEN_PARAM, id);
      return next;
    }, options);
  }

  const index = openId === null ? -1 : order.indexOf(openId);
  const step = (id: string | undefined) =>
    index < 0 || id === undefined ? undefined : () => show(id, { replace: true, state });

  return {
    openId,
    open: (id, nextOrder) => {
      if (nextOrder) followOrder(nextOrder);
      if (id === openId) return;
      // A record already open is replaced, so one Back always closes the panel.
      if (openId !== null) return show(id, { replace: true, state });
      const from = addressWithout(pathname, params);
      show(id, { replace: false, state: { ...stateObject(state), [PUSHED_FROM]: from } });
    },
    close: () => {
      if (pushedFrom(state) === addressWithout(pathname, params)) void navigate(-1);
      else show(null, { replace: true, state: withoutPushed(state) });
    },
    followOrder,
    previous: step(order[index - 1]),
    next: step(order[index + 1]),
  };
}
