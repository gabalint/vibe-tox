import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Timestamp } from "@/components/Timestamp";
import type { RestoreAvailability } from "@/generated/RestoreAvailability";
import { useCommandQuery } from "@/lib/commands";
import styles from "./auth.module.css";
import { GET_RESTORE_AVAILABILITY } from "./contract";

/** Why the engine refused to apply a confirmed restore at start; an unknown code shows as sent. */
const APPLY_FAILURES: Record<string, string> = {
  staged_database_changed: "The checked backup copy changed or went missing before it was applied.",
  users_exist: "User accounts were created on this computer before the restore was applied.",
};

interface RestoreEntryProps {
  /** Keep active restore notices above account creation; the alternative action can follow it. */
  part: "notices" | "action";
  /** Opens the restore flow; absent when the app frame provides none. */
  onStart?: (() => void) | undefined;
}

/**
 * On the setup screen: the state of a restore from a backup, and the way into one while the
 * engine offers it (M4 design brief §3.7). A restore the engine could not apply stays reported
 * instead of the confirmed one, whose marker the engine keeps. Says nothing when the engine cannot be asked, so
 * setting up a new laboratory always stays possible.
 */
export function RestoreEntry({ onStart, part }: RestoreEntryProps) {
  const availability = useCommandQuery<RestoreAvailability>(GET_RESTORE_AVAILABILITY);
  if (!availability.isSuccess) return null;
  const { available, pending, lastFailureCode } = availability.data;
  return (
    <>
      {part === "notices" && pending && !lastFailureCode && (
        <Alert tone="info" title="A restore is ready to finish">
          A restore from the backup made at <Timestamp value={pending.setCreatedAt} /> is ready.
          Close the app and open it again to finish.
        </Alert>
      )}
      {part === "notices" && lastFailureCode && (
        <Alert tone="warning" title="The last restore could not be applied">
          {APPLY_FAILURES[lastFailureCode] ?? `Code: ${lastFailureCode}`} The current data was kept.
        </Alert>
      )}
      {part === "action" && available && onStart && (
        <div className={styles.flow}>
          <h2 className={styles.stepTitle}>Replacing a laboratory computer?</h2>
          <p>
            Instead of setting up a new laboratory, restore its data from a backup. You need the
            recovery kit.
          </p>
          <div>
            <Button onPress={onStart}>Restore from a backup</Button>
          </div>
        </div>
      )}
    </>
  );
}
