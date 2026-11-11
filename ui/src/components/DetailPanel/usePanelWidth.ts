import {
  createContext,
  type KeyboardEvent,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { useLocation } from "react-router";
import { useResizeDrag } from "@/lib/useResizeDrag";

const STORAGE_KEY = "vibe-tox.side-panel.widths-v1";
/** The panel's narrowest width, its default upper bound and its widest width, in rem. */
export const PANEL_MIN_REM = 22;
const PANEL_DEFAULT_MAX_REM = 34;
const PANEL_MAX_REM = 64;
/** The default width's share of the layout, as before resizing existed. */
const PANEL_DEFAULT_SHARE = 0.35;
const STEP = 32;

const clamp = (value: number, min: number, max: number) =>
  Math.round(Math.min(max, Math.max(min, value)));

/** Saved widths in pixels, one per page path. */
function readWidths(): Record<string, number> {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    if (typeof saved !== "object" || saved === null) return {};
    return Object.fromEntries(
      Object.entries(saved).filter((entry): entry is [string, number] => Number.isFinite(entry[1])),
    );
  } catch {
    // Preferences are optional: a damaged or unavailable store must not prevent the panel.
    return {};
  }
}

/**
 * Asks the panel to be as wide as it may while `active`, such as for a wide table; it returns
 * to its previous width when no content asks any more. Does nothing outside a `DetailPanel`.
 */
export const WidePanelContext = createContext<(() => () => void) | null>(null);

export function useWidePanel(active: boolean) {
  const request = useContext(WidePanelContext);
  useEffect(() => (active ? request?.() : undefined), [active, request]);
}

/**
 * The side panel's width beside the list: dragged or set from the keyboard, remembered on this
 * computer for each page. `available` is the layout's width and `rem` its root font size, in
 * pixels; `listMinRem` keeps the list's narrowest useful width beside the panel.
 */
export function usePanelWidth(available: number, rem: number, listMinRem: number) {
  const path = useLocation().pathname;
  const [chosen, setChosen] = useState<number | null>(() => readWidths()[path] ?? null);
  const [wide, setWide] = useState<{ count: number; width: number | null }>({
    count: 0,
    width: null,
  });
  const min = PANEL_MIN_REM * rem;
  // Without a measured layout (test environments), the list's room is unknown.
  const max = Math.max(
    min,
    available ? Math.min(PANEL_MAX_REM * rem, available - listMinRem * rem) : PANEL_MAX_REM * rem,
  );
  const fallback = Math.min(
    PANEL_DEFAULT_MAX_REM * rem,
    available ? PANEL_DEFAULT_SHARE * available : Number.POSITIVE_INFINITY,
  );
  const size = clamp(wide.count > 0 ? (wide.width ?? max) : (chosen ?? fallback), min, max);

  const resize = (next: number) => {
    const width = clamp(next, min, max);
    if (wide.count > 0) setWide((last) => ({ ...last, width }));
    else setChosen(width);
  };
  const { dragging, pointerProps } = useResizeDrag({
    size,
    axis: "x",
    direction: -1,
    onResize: resize,
    onCancel: () => {
      setChosen(chosen);
      setWide(wide);
    },
  });

  // Persist after a drag ends, not on every pointer movement; a widened panel is not a choice.
  useEffect(() => {
    if (dragging || chosen === null) return;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...readWidths(), [path]: chosen }));
    } catch {
      // Continue with the in-memory width when this webview cannot persist it.
    }
  }, [path, chosen, dragging]);

  const requestWide = useCallback(() => {
    setWide((last) => ({ ...last, count: last.count + 1 }));
    return () =>
      setWide((last) =>
        last.count <= 1 ? { count: 0, width: null } : { ...last, count: last.count - 1 },
      );
  }, []);

  function onKeyDown(event: KeyboardEvent) {
    const sizes: Record<string, number> = {
      ArrowLeft: size + STEP,
      ArrowRight: size - STEP,
      Home: min,
      End: max,
    };
    const next = sizes[event.key];
    if (next === undefined) return;
    event.preventDefault();
    resize(next);
  }

  return {
    size,
    min: Math.round(min),
    max: Math.round(max),
    dragging,
    requestWide,
    separatorProps: { ...pointerProps, onKeyDown },
  };
}

export type PanelWidth = ReturnType<typeof usePanelWidth>;

/** The layout's panel width, for the `DetailPanel` inside it; null outside a `DetailLayout`. */
export const PanelWidthContext = createContext<PanelWidth | null>(null);
