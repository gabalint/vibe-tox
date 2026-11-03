import type { ReactNode } from "react";
import styles from "./Form.module.css";

/** A button in a `FieldGrid`, lined up with the inputs beside it rather than their labels. */
export function FieldAction({ children }: { children: ReactNode }) {
  return <div className={styles.fieldAction}>{children}</div>;
}
