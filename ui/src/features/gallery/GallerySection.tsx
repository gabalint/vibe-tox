import type { ReactNode } from "react";
import styles from "./gallery.module.css";

export function GallerySection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.section} aria-label={title}>
      <h2 className={styles.heading}>{title}</h2>
      {children}
    </section>
  );
}
