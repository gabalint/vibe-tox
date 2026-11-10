import { useBlocker } from "react-router";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useUnsavedForm } from "./unsavedForms";

/**
 * Asks before leaving a full-page form with typed input that is not saved, such as through
 * the navigation. Pass `when={isDirty && !isSubmitSuccessful}` from react-hook-form's form
 * state, so a saved form leaves freely. `form` is the page's name, such as "New order"; sign
 * out and closing the app name it in their warnings. Needs the app's data router. In `lib/`
 * because it knows the router; it does not call the engine.
 */
export function UnsavedChangesGuard({ when, form }: { when: boolean; form: string }) {
  const blocker = useBlocker(when);
  useUnsavedForm(form, when);
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
