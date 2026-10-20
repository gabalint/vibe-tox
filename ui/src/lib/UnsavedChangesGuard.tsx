import { useBlocker } from "react-router";
import { ConfirmDialog } from "@/components/ConfirmDialog";

/**
 * Asks before leaving a full-page form with typed input that is not saved, such as through
 * the navigation. Pass `when={isDirty && !isSubmitSuccessful}` from react-hook-form's form
 * state, so a saved form leaves freely. Needs the app's data router. In `lib/` because it
 * knows the router; it does not call the engine.
 */
export function UnsavedChangesGuard({ when }: { when: boolean }) {
  const blocker = useBlocker(when);
  return (
    <ConfirmDialog
      title="Leave without saving?"
      isOpen={blocker.state === "blocked"}
      onOpenChange={(isOpen) => {
        if (!isOpen) blocker.reset?.();
      }}
      confirmLabel="Leave without saving"
      onConfirm={() => blocker.proceed?.()}
    >
      <p>What you typed on this page is not saved. If you leave, it is lost.</p>
    </ConfirmDialog>
  );
}
