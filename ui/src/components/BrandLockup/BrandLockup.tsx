// The desktop icon is the one source of the mark.
import mark from "../../../../src-tauri/app-icon.svg";
import styles from "./BrandLockup.module.css";

/**
 * The mark with the short product name over its parent project. The full name, Steady State
 * LIMS, stays in window titles, bundles and documents (docs/development/packaging.md).
 */
export function BrandLockup() {
  return (
    <span className={styles.lockup}>
      <img className={styles.mark} src={mark} alt="" />
      <span className={styles.text}>
        {/* The space separates the two lines for screen readers; the grid ignores it. */}
        <span className={styles.product}>Steady State</span>{" "}
        <span className={styles.parent}>An OpenBGA project</span>
      </span>
    </span>
  );
}
