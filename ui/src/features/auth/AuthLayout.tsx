import type { ReactNode } from "react";
import { BrandLockup } from "@/components/BrandLockup";
import styles from "./auth.module.css";

/** The centered frame of the signed-out screens. */
export function AuthLayout({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <main className={styles.layout}>
      <div className={styles.card}>
        <p className={styles.appName}>
          <BrandLockup />
        </p>
        {children}
      </div>
      {aside && <div className={styles.aside}>{aside}</div>}
    </main>
  );
}
