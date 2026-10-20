import { Alert } from "@/components/Alert";
import type { CommandError } from "@/generated/CommandError";
import { toUserMessage } from "@/lib/errors";
import styles from "@/styles/layout.module.css";

interface CommandRefusalProps {
  title: string;
  error: CommandError;
  /** What the refusal means and what to do next, in the caller's workflow. */
  lead: string | null;
}

/**
 * An engine refusal: what it means, the engine's message naming the
 * records, and the support reference. `ErrorState` would add role guidance to every
 * `forbidden`, which is wrong when the reviewer authored drafts.
 */
export function CommandRefusal({ title, error, lead }: CommandRefusalProps) {
  return (
    <Alert tone="danger" title={title}>
      {lead && <p>{lead}</p>}
      <p>{toUserMessage(error)}</p>
      {error.correlationId && (
        <p className={styles.hint}>
          Support reference: <span className={styles.code}>{error.correlationId}</span>
        </p>
      )}
    </Alert>
  );
}
