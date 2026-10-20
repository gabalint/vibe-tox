import { type KeyboardEvent, type RefObject, useEffect, useLayoutEffect, useState } from "react";
import { useResizeDrag } from "@/lib/useResizeDrag";

const STORAGE_KEY = "vibe-tox.help.layout-v1";
const DEFAULT = { width: 400, height: 320 };
const STEP = 32;
const STACKED = "(max-width: 85rem)";
const clamp = (value: number, min: number, max: number) =>
  Math.round(Math.min(max, Math.max(min, value)));

function readSizes() {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    return {
      width: Number.isFinite(saved?.width) ? clamp(saved.width, 320, 800) : DEFAULT.width,
      height: Number.isFinite(saved?.height)
        ? Math.max(160, Math.round(saved.height))
        : DEFAULT.height,
    };
  } catch {
    return DEFAULT;
  }
}

/** Measure the actual working row, so banners and navigation cannot push help off-screen. */
function useHelpBounds(
  main: RefObject<HTMLElement | null>,
  pane: RefObject<HTMLElement | null>,
  open: boolean,
) {
  const [bounds, setBounds] = useState({
    stacked: window.matchMedia?.(STACKED).matches ?? false,
    available: 1000,
  });
  useLayoutEffect(() => {
    const media = window.matchMedia?.(STACKED);
    function measure() {
      const stacked = media?.matches ?? false;
      const primary = main.current?.getBoundingClientRect();
      const help = pane.current?.getBoundingClientRect();
      const dimension = stacked ? "height" : "width";
      const available = (primary?.[dimension] ?? 0) + (open ? (help?.[dimension] ?? 0) : 0);
      // A hidden/testing surface has no geometry; retain useful defaults until it is measured.
      setBounds((last) => {
        const next = { stacked, available: available || last.available };
        return next.stacked === last.stacked && next.available === last.available ? last : next;
      });
    }
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    if (main.current) observer?.observe(main.current);
    if (pane.current) observer?.observe(pane.current);
    media?.addEventListener("change", measure);
    window.addEventListener("resize", measure);
    measure();
    return () => {
      window.removeEventListener("resize", measure);
      observer?.disconnect();
      media?.removeEventListener("change", measure);
    };
  }, [main, pane, open]);
  const max = Math.floor(
    bounds.stacked ? bounds.available * 0.6 : Math.min(800, bounds.available - 480),
  );
  return {
    stacked: bounds.stacked,
    min: Math.min(bounds.stacked ? 160 : 320, Math.max(0, max)),
    max: Math.max(0, max),
  };
}

export function useHelpLayout(
  main: RefObject<HTMLElement | null>,
  pane: RefObject<HTMLElement | null>,
  open: boolean,
) {
  const [sizes, setSizes] = useState(readSizes);
  const { stacked, min, max } = useHelpBounds(main, pane, open);
  const dimension = stacked ? "height" : "width";
  const size = clamp(sizes[dimension], min, max);
  const resize = (value: number) =>
    setSizes((last) => ({ ...last, [dimension]: clamp(value, min, max) }));
  const { dragging, pointerProps } = useResizeDrag({
    size,
    axis: stacked ? "y" : "x",
    direction: -1,
    onResize: resize,
    onCancel: () => setSizes(sizes),
  });
  useEffect(() => {
    if (dragging) return;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(sizes));
    } catch {
      /* Optional UI preference. */
    }
  }, [sizes, dragging]);

  function onKeyDown(event: KeyboardEvent) {
    const larger = stacked ? "ArrowUp" : "ArrowLeft";
    const smaller = stacked ? "ArrowDown" : "ArrowRight";
    const next = { [larger]: size + STEP, [smaller]: size - STEP, Home: min, End: max }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    resize(next);
  }

  return {
    size,
    min,
    max,
    stacked,
    dragging,
    smaller: () => resize(size - STEP),
    larger: () => resize(size + STEP),
    reset: () => resize(DEFAULT[dimension]),
    separatorProps: { ...pointerProps, onKeyDown },
  };
}
