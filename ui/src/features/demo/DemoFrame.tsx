import type { ReactNode } from "react";
import { useInstallation } from "./contract";
import { DemoBanner } from "./DemoBanner";
import styles from "./DemoFrame.module.css";

/**
 * Puts the demo banner above every screen of the demo laboratory, signed in or not, and even
 * when its engine did not start; a laboratory installation's screens are left as they are.
 */
export function DemoFrame({ children }: { children: ReactNode }) {
  if (useInstallation() !== "demo") return children;
  return (
    <div className={styles.frame}>
      <DemoBanner />
      <div className={styles.screen}>{children}</div>
    </div>
  );
}
