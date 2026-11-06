import { type Tone, toneIcons } from "@/components/tones";
import styles from "./StatusBadge.module.css";

interface StatusBadgeProps {
  tone: Tone;
  /** The status in words; required, because color alone never carries status. */
  children: string;
  /** Fills its cell, the label centered, so a column of statuses lines up. */
  fill?: boolean;
}

/** A compact status label. Callers map their states to a tone; this only displays it. */
export function StatusBadge({ tone, children, fill = false }: StatusBadgeProps) {
  const Icon = toneIcons[tone];
  return (
    <span className={styles.badge} data-tone={tone} data-fill={fill || undefined}>
      <Icon aria-hidden="true" className={styles.icon} />
      {children}
    </span>
  );
}
