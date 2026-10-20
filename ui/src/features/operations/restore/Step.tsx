import type { ReactNode } from "react";
import layout from "@/styles/layout.module.css";
import styles from "./restore.module.css";

/** One step of the restore flow, under its `h2`. */
export function Step({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className={layout.stack}>
      <h2 className={styles.stepTitle}>{title}</h2>
      {children}
    </div>
  );
}
