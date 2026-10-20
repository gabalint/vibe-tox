import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import type { RestoreSessionView } from "@/generated/RestoreSessionView";
import type { StagedRestoreView } from "@/generated/StagedRestoreView";
import { AnchorCheck, useAnchorCheck } from "./AnchorCheck";
import { ConfirmRestoreDialog } from "./ConfirmRestoreDialog";
import { RestoreActions } from "./RestoreActions";
import { StagedSummary } from "./StagedSummary";
import { Step } from "./Step";

interface ReviewStepProps {
  session: RestoreSessionView;
  staged: StagedRestoreView;
  onClosing: () => void;
  onExit: () => void;
}

/**
 * The result of the check. The restore is offered only when the engine says it may go ahead
 * (`canConfirm`); otherwise the findings say why not.
 */
export function ReviewStep({ session, staged, onClosing, onExit }: ReviewStepProps) {
  const { restoreSessionId } = session;
  const [isConfirming, setConfirming] = useState(false);
  const anchorCheck = useAnchorCheck();
  return (
    <Step title="Result of the check">
      <StagedSummary staged={staged} />
      <AnchorCheck restoreSessionId={restoreSessionId} check={anchorCheck} />
      {!staged.canConfirm && (
        <Alert tone="warning" title="This backup cannot be restored">
          See the findings above. Cancel the restore and choose another backup.
        </Alert>
      )}
      <RestoreActions
        restoreSessionId={restoreSessionId}
        onExit={onExit}
        isBusy={anchorCheck.isPending}
      >
        {staged.canConfirm && (
          <Button
            variant="danger"
            isDisabled={anchorCheck.isPending}
            onPress={() => setConfirming(true)}
          >
            Restore this backup
          </Button>
        )}
      </RestoreActions>
      {isConfirming && (
        <ConfirmRestoreDialog
          restoreSessionId={restoreSessionId}
          onClosing={onClosing}
          onClose={() => setConfirming(false)}
        />
      )}
    </Step>
  );
}
