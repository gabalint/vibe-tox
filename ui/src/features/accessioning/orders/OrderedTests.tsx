import { type RefObject, useEffect, useRef, useState } from "react";
import {
  Tooltip,
  type TooltipTriggerState,
  TooltipTriggerStateContext,
  VisuallyHidden,
} from "react-aria-components";
import type { OrderSummary } from "@/generated/OrderSummary";
import styles from "./OrderedTests.module.css";

/** The tooltip title: "1 test ordered", "4 tests ordered". */
const testsOrdered = (count: number) => `${count} ${count === 1 ? "test" : "tests"} ordered`;

/**
 * Open while the pointer is on `ref` or its grid cell has keyboard focus; the grid keeps
 * focus on the cell, not on its content, so `TooltipTrigger` cannot serve. Escape closes
 * it until the next hover or focus.
 */
function useTooltipState(ref: RefObject<HTMLElement | null>): TooltipTriggerState {
  const [open, setOpen] = useState(false);
  // Subscribes to the focus of the grid cell, which the grid renders around this content.
  useEffect(() => {
    const cell = ref.current?.closest<HTMLElement>('[role="gridcell"]');
    if (!cell) return;
    const show = () => setOpen(true);
    const hide = () => setOpen(false);
    cell.addEventListener("focus", show);
    cell.addEventListener("blur", hide);
    return () => {
      cell.removeEventListener("focus", show);
      cell.removeEventListener("blur", hide);
    };
  }, [ref]);
  // Subscribes to Escape while open, so the tooltip can be dismissed without moving.
  useEffect(() => {
    if (!open) return;
    const dismiss = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("keydown", dismiss);
    return () => document.removeEventListener("keydown", dismiss);
  }, [open]);
  return {
    isOpen: open,
    shouldSkipAnimation: true,
    open: () => setOpen(true),
    close: () => setOpen(false),
  };
}

/**
 * The order's test count, marked as hoverable; hovering it or focusing its cell lists the
 * ordered tests, one per line with name and code (#4747). Screen readers hear the list
 * with the cell.
 */
export function OrderedTests({ order }: { order: OrderSummary }) {
  const ref = useRef<HTMLSpanElement>(null);
  const state = useTooltipState(ref);
  const title = testsOrdered(order.itemCount);
  const tests = order.orderedTests.map((test) => `${test.name} (${test.code})`);
  return (
    <>
      <span
        ref={ref}
        aria-hidden="true"
        className={styles.count}
        onPointerEnter={() => state.open()}
        onPointerLeave={() => state.close()}
      >
        {order.itemCount}
      </span>
      <VisuallyHidden>{`${title}: ${tests.join("; ")}`}</VisuallyHidden>
      <TooltipTriggerStateContext.Provider value={state}>
        <Tooltip triggerRef={ref} placement="bottom end" className={styles.tooltip}>
          <div className={styles.title}>{title}</div>
          <ul className={styles.tests}>
            {tests.map((test, index) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: an order can list a test twice; the list is fixed while shown.
              <li key={index}>{test}</li>
            ))}
          </ul>
        </Tooltip>
      </TooltipTriggerStateContext.Provider>
    </>
  );
}
