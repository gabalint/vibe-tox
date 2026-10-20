import { FormProvider, useForm } from "react-hook-form";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { type TextFormat, TextInput } from "@/components/FormFields";
import type { ConfirmRestoreInput } from "@/generated/ConfirmRestoreInput";
import type { ConfirmRestoreView } from "@/generated/ConfirmRestoreView";
import { useCommandMutation } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { CONFIRM_RESTORE } from "../contract";

const CONFIRMATION_FORMAT: TextFormat = {
  pattern: /^RESTORE$/,
  message: "Type RESTORE in capital letters.",
  description: "Type RESTORE in capital letters.",
};

interface ConfirmRestoreDialogProps {
  restoreSessionId: string;
  onClosing: () => void;
  onClose: () => void;
}

/** The last step: the engine records the restore to apply at the next start, then the app quits. */
export function ConfirmRestoreDialog({
  restoreSessionId,
  onClosing,
  onClose,
}: ConfirmRestoreDialogProps) {
  const confirm = useCommandMutation<ConfirmRestoreView, { input: ConfirmRestoreInput }>(
    CONFIRM_RESTORE,
    { invalidates: [] },
  );
  const form = useForm<{ confirmation: string }>({ defaultValues: { confirmation: "" } });
  const onConfirm = form.handleSubmit(({ confirmation }) =>
    confirm.mutate(
      { input: { restoreSessionId, confirmation } },
      {
        onSuccess: onClosing,
        onError: (error) => {
          for (const each of error.fieldErrors) {
            if (each.field === "confirmation") {
              form.setError("confirmation", { message: each.message });
            }
          }
        },
      },
    ),
  );
  return (
    <ConfirmDialog
      title="Restore this backup"
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel="Restore and close the app"
      onConfirm={() => void onConfirm()}
      isPending={confirm.isPending}
      variant="danger"
    >
      <div className={layout.stack}>
        <p>
          The current data folder is kept aside; the app closes; open it again to finish. The
          laboratory's data then comes from this backup, and a recovery reconciliation opens.
        </p>
        <FormProvider {...form}>
          <TextInput
            name="confirmation"
            label="Confirmation"
            format={CONFIRMATION_FORMAT}
            isRequired
          />
        </FormProvider>
        {confirm.error && <ErrorState title="The restore was not prepared" error={confirm.error} />}
      </div>
    </ConfirmDialog>
  );
}
