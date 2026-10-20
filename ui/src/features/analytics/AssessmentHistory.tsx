import type { ReactNode } from "react";
import styles from "@/styles/layout.module.css";

interface AssessmentHistoryProps<Item> {
  current: Item | null;
  /** Every assessment of the same scope, the current one included. */
  history: readonly Item[];
  getId: (item: Item) => string;
  render: (item: Item) => ReactNode;
  /** Shown when nothing is assessed yet. */
  emptyText: string;
}

/**
 * The current assessment, then the earlier ones it replaced. Assessments are append-only:
 * an earlier one stays readable after it is replaced (REQ-ANA-005, REQ-SPC-008).
 */
export function AssessmentHistory<Item>({
  current,
  history,
  getId,
  render,
  emptyText,
}: AssessmentHistoryProps<Item>) {
  const currentId = current === null ? null : getId(current);
  const earlier = history.filter((item) => getId(item) !== currentId);
  return (
    <div className={styles.stack}>
      <div>
        <h3 className={styles.subheading}>Current assessment</h3>
        {current === null ? <p>{emptyText}</p> : render(current)}
      </div>
      {earlier.length > 0 && (
        <div>
          <h3 className={styles.subheading}>Earlier assessments</h3>
          <ul className={styles.list}>
            {earlier.map((item) => (
              <li key={getId(item)}>{render(item)}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
