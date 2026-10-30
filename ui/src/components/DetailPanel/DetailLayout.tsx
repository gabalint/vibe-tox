import {
  createContext,
  type ReactNode,
  type RefObject,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import styles from "./DetailPanel.module.css";
import { PANEL_MIN_REM, PanelWidthContext, usePanelWidth, WidePanelContext } from "./usePanelWidth";

/**
 * The list's narrowest useful width beside the panel, in rem: "about 40rem" (UI standards,
 * side panel). With the 14rem navigation and the page's 1.5rem padding, a 1280px window gives
 * the layout 63rem (1008px): the panel at its 22rem minimum, the 1rem gap and a list of
 * 39.95rem (639px) side by side. The panel replaces the list below a 1248px window.
 */
const LIST_MIN_REM = 38;
/** The gap before the panel, in rem; see `.layout` in the CSS. */
const GAP_REM = 1;

/** Whether the panel replaces the list, because both do not fit side by side. */
export const NarrowLayoutContext = createContext(false);

interface DetailLayoutProps {
  /** The open record's `DetailPanel`; null or undefined while no record is open. */
  panel?: ReactNode;
  /** The list: its toolbar and a `DataGrid` with `fill`, which takes the remaining height. */
  children: ReactNode;
}

/**
 * A list with the open record's side panel docked on its right, inside `Page`. When the
 * list would be narrower than about 40rem (38rem) beside the panel, the panel replaces the
 * list and offers "Back to list". The list is hidden, not removed, so its sort and scroll survive.
 * Beside the list, the panel's width can be dragged between its minimum and the width that
 * leaves the list its 38rem.
 */
export function DetailLayout({ panel, children }: DetailLayoutProps) {
  const ref = useRef<HTMLDivElement>(null);
  const { width, rem } = useWidth(ref);
  const narrow = width > 0 && width < (LIST_MIN_REM + PANEL_MIN_REM + GAP_REM) * rem;
  const panelWidth = usePanelWidth(width, rem, LIST_MIN_REM + GAP_REM);
  return (
    <div ref={ref} className={styles.layout}>
      <div className={styles.list} hidden={narrow && Boolean(panel)}>
        {children}
      </div>
      <NarrowLayoutContext value={narrow}>
        <PanelWidthContext value={panelWidth}>
          <WidePanelContext value={panelWidth.requestWide}>{panel}</WidePanelContext>
        </PanelWidthContext>
      </NarrowLayoutContext>
    </div>
  );
}

/** The element's width and the root font size, in pixels; width 0 until it is measured. */
function useWidth(ref: RefObject<HTMLElement | null>) {
  const [size, setSize] = useState({ width: 0, rem: 16 });
  // Synchronizes with the element's width, which lives outside React. Without
  // ResizeObserver (test environments), the layout stays side by side.
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      setSize({ width: entry.contentRect.width, rem });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return size;
}
