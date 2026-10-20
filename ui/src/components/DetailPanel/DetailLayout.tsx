import {
  createContext,
  type ReactNode,
  type RefObject,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import styles from "./DetailPanel.module.css";

/**
 * The list's narrowest useful width beside the panel, in rem: "about 40rem" (UI standards,
 * side panel). With the 14rem navigation and the page's 1.5rem padding, a 1280px window gives
 * the layout 63rem (1008px): the panel at its 22rem minimum, the 1rem gap and a list of
 * 39.95rem (639px) side by side. The panel replaces the list below a 1248px window.
 */
const LIST_MIN_REM = 38;
/** The panel's narrowest width and the gap before it, in rem; see `.panel` in the CSS. */
const PANEL_MIN_REM = 22;
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
 */
export function DetailLayout({ panel, children }: DetailLayoutProps) {
  const ref = useRef<HTMLDivElement>(null);
  const narrow = useNarrowWidth(ref);
  return (
    <div ref={ref} className={styles.layout}>
      <div className={styles.list} hidden={narrow && Boolean(panel)}>
        {children}
      </div>
      <NarrowLayoutContext value={narrow}>{panel}</NarrowLayoutContext>
    </div>
  );
}

/** Whether the element is too narrow for the list and the panel side by side. */
function useNarrowWidth(ref: RefObject<HTMLElement | null>): boolean {
  const [narrow, setNarrow] = useState(false);
  // Synchronizes with the element's width, which lives outside React. Without
  // ResizeObserver (test environments), the layout stays side by side.
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      setNarrow(entry.contentRect.width < (LIST_MIN_REM + PANEL_MIN_REM + GAP_REM) * rem);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return narrow;
}
