import { type ReactNode, type RefObject, useLayoutEffect, useRef } from "react";
import styles from "./Banner.module.css";

/** One fact of the strip. */
interface BannerItem {
  /** Unique within the strip. */
  label: string;
  value: ReactNode;
  /** The one fact read first, shown larger. */
  emphasis?: boolean;
}

interface BannerProps {
  /** Names the region, such as "Patient identity". */
  label: string;
  items: readonly BannerItem[];
  /** Status shown after the facts, such as a `StatusBadge`. */
  status?: ReactNode;
}

/**
 * A dense strip of facts about what a screen is about, as a labelled region. It stays in
 * view while the screen scrolls (sticky at the top of the scrolling main area), and the
 * browser never scrolls a focused element under it.
 */
export function Banner({ label, items, status }: BannerProps) {
  const ref = useRef<HTMLElement>(null);
  useScrollPaddingBelow(ref);
  return (
    <section ref={ref} aria-label={label} className={styles.banner}>
      <dl className={styles.items}>
        {items.map((item) => (
          <div key={item.label} className={styles.item} data-emphasis={item.emphasis || undefined}>
            <dt className={styles.label}>{item.label}</dt>
            <dd className={styles.value}>{item.value}</dd>
          </div>
        ))}
      </dl>
      {status && <div className={styles.status}>{status}</div>}
    </section>
  );
}

function scrolls(element: Element): boolean {
  const style = getComputedStyle(element);
  return /auto|scroll/.test(`${style.overflowY} ${style.overflow}`);
}

/**
 * Keeps the top scroll padding of the banner's scroll container at the banner's height, so
 * focusing an element above the fold scrolls it below the banner, not under it (WCAG 2.2
 * SC 2.4.11, technique C43). The banner's height changes with its facts and the window.
 */
function useScrollPaddingBelow(ref: RefObject<HTMLElement | null>) {
  // Synchronizes the container's style, outside React, with the banner's size. Without
  // ResizeObserver (test environments) nothing is set.
  useLayoutEffect(() => {
    const banner = ref.current;
    let container = banner?.parentElement ?? null;
    while (container && !scrolls(container)) container = container.parentElement;
    if (!banner || !container || typeof ResizeObserver === "undefined") return;
    const target = container;
    const observer = new ResizeObserver(() => {
      const height = banner.getBoundingClientRect().height;
      target.style.scrollPaddingTop = `calc(${height}px + var(--space-2))`;
    });
    observer.observe(banner);
    return () => {
      observer.disconnect();
      target.style.scrollPaddingTop = "";
    };
  }, [ref]);
}
