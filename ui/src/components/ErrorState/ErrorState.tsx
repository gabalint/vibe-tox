import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import type { CommandError } from "@/generated/CommandError";
import { toUserMessage } from "@/lib/errors";
import styles from "./ErrorState.module.css";
import { GateReasons } from "./GateReasons";

interface ErrorStateProps {
  title?: string;
  error: CommandError;
  onRetry?: () => void;
}

/** What to do when the engine answers `forbidden`: trying again cannot help. */
const FORBIDDEN_GUIDANCE =
  "Your roles do not include this work. A laboratory administrator can change roles.";

/**
 * A failed engine call: the safe message, each gate that blocked the action, and the
 * support reference that finds the matching log line (omitted when the failure never
 * reached the engine). A `forbidden` answer says who can help instead of offering a retry.
 */
export function ErrorState({
  title = "This could not be loaded",
  error,
  onRetry,
}: ErrorStateProps) {
  const forbidden = error.code === "forbidden";
  return (
    <Alert tone="danger" title={title}>
      <p>{toUserMessage(error)}</p>
      {forbidden && <p>{FORBIDDEN_GUIDANCE}</p>}
      {error.gateReasons.length > 0 && <GateReasons label="Blocked by" gates={error.gateReasons} />}
      {error.correlationId && (
        <p className={styles.reference}>
          Support reference: <span className={styles.id}>{error.correlationId}</span>
        </p>
      )}
      {onRetry && !forbidden && (
        <div className={styles.retry}>
          <Button onPress={onRetry}>Try again</Button>
        </div>
      )}
    </Alert>
  );
}
