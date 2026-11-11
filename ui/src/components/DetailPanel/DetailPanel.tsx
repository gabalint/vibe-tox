import { ArrowLeft, ChevronDown, ChevronUp, X } from "lucide-react";
import {
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { Link } from "react-router";
import { Button } from "@/components/Button";
import { NarrowLayoutContext } from "./DetailLayout";
import styles from "./DetailPanel.module.css";
import { type PanelWidth, PanelWidthContext } from "./usePanelWidth";

interface DetailPanelProps {
  /** The record's identifier: the panel's `h2` and the region's name. */
  identifier: string;
  /** The record's status beside the identifier, such as a `StatusBadge`. */
  status?: ReactNode;
  /** The record's allowed actions: a primary button, up to two more, then `MoreActions`. */
  actions?: ReactNode;
  /** The record's page, when it also has one; shows "Open page". */
  pageHref?: string;
  /** Closes the panel: Close, Escape and "Back to list" call it. */
  onClose: () => void;
  /** Opens the previous record in the list's order; Previous is disabled without it. */
  onPrevious?: (() => void) | undefined;
  /** Opens the next record in the list's order; Next is disabled without it. */
  onNext?: (() => void) | undefined;
  /** The record in view mode: facts, and at most one short child list. */
  children: ReactNode;
}

/**
 * One record beside its list, in a `DetailLayout`: nonmodal, so the list stays usable.
 * Records open in view mode and every change goes through a dialog from `actions`, so the
 * panel holds no input of its own and needs no unsaved-input guard. Escape closes it when
 * focus is in it and no menu or dialog is open. Previous and Next stay in place, disabled
 * at the ends of the list, so repeated presses never land on another control.
 */
export function DetailPanel(props: DetailPanelProps) {
  const { identifier, onClose, onPrevious, onNext } = props;
  const headingId = useId();
  const narrow = useContext(NarrowLayoutContext);
  const width = useContext(PanelWidthContext);
  const { panelRef, headingRef } = useKeepFocusInPanel(narrow, identifier);

  function closeOnEscape(event: KeyboardEvent<HTMLElement>) {
    // Menus and dialogs handle their own Escape; events from their portals are not ours.
    const inPanel = event.target instanceof Node && event.currentTarget.contains(event.target);
    if (event.key !== "Escape" || event.defaultPrevented || !inPanel) return;
    event.preventDefault();
    onClose();
  }

  return (
    // biome-ignore lint/a11y/noNoninteractiveElementInteractions: Escape from any control inside closes the panel.
    <section
      ref={panelRef}
      aria-labelledby={headingId}
      className={styles.panel}
      data-resizing={width?.dragging || undefined}
      // Runtime geometry from the resize control; all visual styling stays in the CSS module.
      style={width ? ({ "--panel-width": `${width.size}px` } as CSSProperties) : undefined}
      onKeyDown={closeOnEscape}
    >
      {width && !narrow && <PanelSplitter width={width} identifier={identifier} />}
      <header className={styles.header}>
        {narrow && (
          <Button variant="quiet" onPress={onClose}>
            <ArrowLeft aria-hidden="true" />
            Back to list
          </Button>
        )}
        <div className={styles.titleRow}>
          <h2 id={headingId} ref={headingRef} tabIndex={-1} className={styles.title}>
            {identifier}
          </h2>
          {props.status}
          <PanelTools
            pageHref={props.pageHref}
            onPrevious={onPrevious}
            onNext={onNext}
            onClose={narrow ? undefined : onClose}
          />
        </div>
        {props.actions && <div className={styles.actions}>{props.actions}</div>}
      </header>
      <PanelBody identifier={identifier}>{props.children}</PanelBody>
    </section>
  );
}

/** The panel's left edge: drag it, or use the arrow keys, to change the panel's width. */
function PanelSplitter({ width, identifier }: { width: PanelWidth; identifier: string }) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: A focusable ARIA window splitter has drag and keyboard behavior; it is not a static hr.
    <div
      role="separator"
      tabIndex={0}
      aria-label={`Width of ${identifier}`}
      aria-description="Use Left and Right arrows to resize; Home for minimum, End for maximum."
      aria-orientation="vertical"
      aria-valuemin={width.min}
      aria-valuemax={width.max}
      aria-valuenow={width.size}
      aria-valuetext={`${width.size} pixels`}
      className={styles.resize}
      {...width.separatorProps}
    />
  );
}

interface PanelToolsProps {
  pageHref: string | undefined;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
  /** Absent when the panel replaces the list, which offers "Back to list" instead. */
  onClose: (() => void) | undefined;
}

/** "Open page", Previous, Next and Close, at the end of the heading row. */
function PanelTools({ pageHref, onPrevious, onNext, onClose }: PanelToolsProps) {
  return (
    <div className={styles.tools}>
      {pageHref && <Link to={pageHref}>Open page</Link>}
      <Button variant="quiet" aria-label="Previous" isDisabled={!onPrevious} onPress={onPrevious}>
        <ChevronUp aria-hidden="true" />
      </Button>
      <Button variant="quiet" aria-label="Next" isDisabled={!onNext} onPress={onNext}>
        <ChevronDown aria-hidden="true" />
      </Button>
      {onClose && (
        <Button variant="quiet" aria-label="Close" onPress={onClose}>
          <X aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}

/**
 * Moves focus to the panel's heading when focus would otherwise be lost: when the panel
 * replaces the list that held it, or when Previous or Next becomes disabled at an end.
 */
function useKeepFocusInPanel(narrow: boolean, identifier: string) {
  const panelRef = useRef<HTMLElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  // Synchronizes DOM focus, which lives outside React; runs again for each record shown.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `identifier` marks a new record.
  useLayoutEffect(() => {
    const active = document.activeElement;
    const inPanel = panelRef.current?.contains(active) ?? false;
    const lost =
      active === null ||
      active === document.body ||
      (active instanceof HTMLButtonElement && active.disabled);
    if (lost || (narrow && !inPanel)) headingRef.current?.focus();
  }, [narrow, identifier]);
  return { panelRef, headingRef };
}

/**
 * The record's facts. When they are taller than the panel, the body is a named region in the
 * Tab order, so it can be scrolled from the keyboard (WCAG 2.1.1); when they fit, it adds no
 * tab stop.
 */
function PanelBody({ identifier, children }: { identifier: string; children: ReactNode }) {
  const bodyRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const scrolls = useOverflows(bodyRef, contentRef);
  return (
    <div
      ref={bodyRef}
      className={styles.body}
      {...(scrolls
        ? { role: "region", "aria-label": `Details of ${identifier}`, tabIndex: 0 }
        : {})}
    >
      <div ref={contentRef}>{children}</div>
    </div>
  );
}

/** Whether `box` is shorter than its content, kept up to date as either changes size. */
function useOverflows(
  box: RefObject<HTMLElement | null>,
  content: RefObject<HTMLElement | null>,
): boolean {
  const [overflows, setOverflows] = useState(false);
  // Synchronizes with the elements' sizes, which live outside React. Without ResizeObserver
  // (test environments), the body is treated as fitting.
  useLayoutEffect(() => {
    const element = box.current;
    if (!element || !content.current || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() =>
      setOverflows(element.scrollHeight > element.clientHeight),
    );
    observer.observe(element);
    observer.observe(content.current);
    return () => observer.disconnect();
  }, [box, content]);
  return overflows;
}
