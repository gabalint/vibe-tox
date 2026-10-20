import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import type { RestoreSessionInput } from "@/generated/RestoreSessionInput";
import type { RestoreSessionView } from "@/generated/RestoreSessionView";
import type { RestoreSourceView } from "@/generated/RestoreSourceView";
import type { StagedRestoreView } from "@/generated/StagedRestoreView";
import type { StageRestoreInput } from "@/generated/StageRestoreInput";
import { useCommandMutation } from "@/lib/commands";
import { CHOOSE_RESTORE_SOURCE, STAGE_RESTORE } from "../contract";
import { BackupChoice } from "./BackupChoice";
import { RestoreActions } from "./RestoreActions";
import { Step } from "./Step";

interface SourceStepProps {
  session: RestoreSessionView;
  onStaged: (staged: StagedRestoreView) => void;
  onExit: () => void;
}

/**
 * The backup folder, chosen in the app's own folder dialog (the screen never sends a path),
 * then one of the backups the engine found there.
 */
export function SourceStep({ session, onStaged, onExit }: SourceStepProps) {
  const { restoreSessionId } = session;
  const [cancelled, setCancelled] = useState(false);
  const [source, setSource] = useState<RestoreSourceView | null>(null);
  const choose = useCommandMutation<RestoreSourceView | null, { input: RestoreSessionInput }>(
    CHOOSE_RESTORE_SOURCE,
    { invalidates: [] },
  );
  const onChoose = () => {
    setCancelled(false);
    choose.mutate(
      { input: { restoreSessionId } },
      {
        onSuccess: (found) => {
          if (found === null) {
            setCancelled(true);
            return;
          }
          setSource(found);
        },
      },
    );
  };
  const stage = useCommandMutation<StagedRestoreView, { input: StageRestoreInput }>(STAGE_RESTORE, {
    invalidates: [],
  });
  const onCheck = (setId: string) =>
    stage.mutate({ input: { restoreSessionId, setId } }, { onSuccess: onStaged });
  const isBusy = choose.isPending || stage.isPending;
  return (
    <Step title="Choose the backup folder">
      <p>
        Choose the folder that holds the backups, such as the backup folder on the network share or
        the removable drive.
      </p>
      {cancelled && (
        <Alert tone="info">The folder dialog was cancelled. No folder was chosen.</Alert>
      )}
      {choose.error && <ErrorState title="The folder could not be read" error={choose.error} />}
      {source && (
        <BackupChoice
          sets={source.sets}
          onCheck={onCheck}
          isChecking={stage.isPending}
          error={stage.error}
        />
      )}
      <RestoreActions restoreSessionId={restoreSessionId} onExit={onExit} isBusy={isBusy}>
        <Button
          variant={source ? "secondary" : "primary"}
          isPending={choose.isPending}
          isDisabled={stage.isPending}
          onPress={onChoose}
        >
          {source ? "Choose another folder" : "Choose the backup folder"}
        </Button>
      </RestoreActions>
    </Step>
  );
}
