import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Toolbar } from "@/components/Toolbar";
import type { RestoreSessionInput } from "@/generated/RestoreSessionInput";
import { useCommandMutation } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { CANCEL_RESTORE } from "../contract";

interface RestoreActionsProps {
  restoreSessionId: string;
  onExit: () => void;
  /** Blocks cancelling while a step's command runs. */
  isBusy?: boolean;
  /** The step's own actions, before "Cancel restore". */
  children?: ReactNode;
}

/**
 * A step's actions and "Cancel restore", which ends the restore session in the engine, removes
 * what it staged and returns to the setup screen. The engine answers `conflict` when there is
 * nothing left to cancel here: the session expired after its 30 minutes, a restore is already
 * being applied, or the installation is no longer new. Leaving is then all that remains.
 */
export function RestoreActions({
  restoreSessionId,
  onExit,
  isBusy = false,
  children,
}: RestoreActionsProps) {
  const cancel = useCommandMutation<null, { input: RestoreSessionInput }>(CANCEL_RESTORE, {
    invalidates: [],
  });
  const onCancel = () =>
    cancel.mutate(
      { input: { restoreSessionId } },
      {
        onSuccess: onExit,
        onError: (error) => {
          if (error.code === "conflict") onExit();
        },
      },
    );
  return (
    <div className={layout.stack}>
      <Toolbar>
        {children}
        <Button isDisabled={isBusy} isPending={cancel.isPending} onPress={onCancel}>
          Cancel restore
        </Button>
      </Toolbar>
      {cancel.error && <ErrorState title="The restore was not cancelled" error={cancel.error} />}
    </div>
  );
}
