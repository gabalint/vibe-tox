import { type ReactNode, useState } from "react";
import { Inset } from "@/components/Inset";
import { Section } from "@/components/Section";
import styles from "./accessioning.module.css";

interface HoldsSectionProps<Hold> {
  title: string;
  /** Shown when there are no holds, such as "No clarification holds." */
  emptyText: string;
  holds: readonly Hold[];
  holdId: (hold: Hold) => string;
  /** One hold; `resolve` opens its resolution dialog. */
  renderHold: (hold: Hold, resolve: () => void) => ReactNode;
  /** The dialog that resolves `hold`, with a reason; mounted only while open. */
  renderResolution: (hold: Hold, close: () => void) => ReactNode;
}

/** Holds on a record, each resolved in its own dialog with a reason for the audit trail. */
export function HoldsSection<Hold>({
  title,
  emptyText,
  holds,
  holdId,
  renderHold,
  renderResolution,
}: HoldsSectionProps<Hold>) {
  const [resolving, setResolving] = useState<Hold | null>(null);
  return (
    <Section title={title}>
      {holds.length === 0 ? (
        <p className={styles.hint}>{emptyText}</p>
      ) : (
        <ul className={styles.list}>
          {holds.map((hold) => (
            <Inset key={holdId(hold)} as="li">
              {renderHold(hold, () => setResolving(hold))}
            </Inset>
          ))}
        </ul>
      )}
      {resolving !== null && renderResolution(resolving, () => setResolving(null))}
    </Section>
  );
}
