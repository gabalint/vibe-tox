import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Toolbar } from "@/components/Toolbar";
import type { AnchorSummary } from "@/generated/AnchorSummary";
import type { RestoreSessionInput } from "@/generated/RestoreSessionInput";
import { useCommandMutation } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { AnchorCounts } from "../AnchorCounts";
import { CHECK_RESTORE_ANCHORS } from "../contract";

/** The comparison's command; the review step holds it, so nothing else runs meanwhile. */
export function useAnchorCheck() {
  return useCommandMutation<AnchorSummary | null, { input: RestoreSessionInput }>(
    CHECK_RESTORE_ANCHORS,
    { invalidates: [] },
  );
}

interface AnchorCheckProps {
  restoreSessionId: string;
  check: ReturnType<typeof useAnchorCheck>;
}

/**
 * Optional: compares the backup's audit trails with anchors kept on another drive, such as a
 * removable drive holding audit anchors only. The folder is chosen in the app's own dialog. The
 * engine checks the whole backup again with those anchors and keeps that result for the
 * confirmation, which it refuses if the anchors disagree with the backup.
 */
export function AnchorCheck({ restoreSessionId, check }: AnchorCheckProps) {
  const [cancelled, setCancelled] = useState(false);
  const [summary, setSummary] = useState<AnchorSummary | null>(null);
  const onPress = () => {
    setCancelled(false);
    check.mutate(
      { input: { restoreSessionId } },
      {
        onSuccess: (found) => {
          setCancelled(found === null);
          if (found !== null) setSummary(found);
        },
      },
    );
  };
  return (
    <>
      <Toolbar>
        <Button isPending={check.isPending} onPress={onPress}>
          Compare with anchors on another drive
        </Button>
      </Toolbar>
      {check.isPending && (
        <Alert tone="info" title="Comparing the anchors">
          The app checks the backup again with the anchors in the chosen folder. This can take
          several minutes.
        </Alert>
      )}
      {cancelled && (
        <Alert tone="info">The folder dialog was cancelled. No anchors were compared.</Alert>
      )}
      {check.error && <ErrorState title="The anchors could not be compared" error={check.error} />}
      {summary && (
        <>
          <h3 className={layout.subheading}>Anchors on the other drive</h3>
          <AnchorCounts summary={summary} />
        </>
      )}
    </>
  );
}
