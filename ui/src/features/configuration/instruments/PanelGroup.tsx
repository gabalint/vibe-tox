import type { ReactNode } from "react";
import styles from "./PanelGroup.module.css";

/**
 * One titled group of the analyzer's panel, open at first. Its title collapses it, so a long
 * group such as the AU settings can be put out of the way.
 */
export function PanelGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <details open className={styles.group}>
      <summary className={styles.summary}>{title}</summary>
      {children}
    </details>
  );
}
