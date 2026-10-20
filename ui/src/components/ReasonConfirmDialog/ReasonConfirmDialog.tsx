import type { ReactNode } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { CommandError } from "@/generated/CommandError";
import styles from "./ReasonConfirmDialog.module.css";

/** The parts of a `useCommandMutation` result the dialog runs. */
interface ReasonCommand<Input> {
  mutate: (args: { input: Input }, options: { onSuccess: () => void }) => void;
  error: CommandError | null;
  isPending: boolean;
}

interface ReasonConfirmDialogProps<Input> {
  title: string;
  confirmLabel: string;
  /** `danger` for decisions that cannot be undone, such as rejecting an order. */
  variant: "primary" | "danger";
  command: ReasonCommand<Input>;
  /** The command's input for the reason given. */
  input: (reason: string) => Input;
  /** The notice once the engine has recorded the decision. */
  successTitle: string;
  onClose: () => void;
  /** What will happen, in laboratory terms. */
  children: ReactNode;
}

/**
 * Confirms a decision that needs a reason for the audit trail, runs its command, and closes
 * once the engine has recorded it. Mount it only while open.
 */
export function ReasonConfirmDialog<Input>({
  title,
  confirmLabel,
  variant,
  command,
  input,
  successTitle,
  onClose,
  children,
}: ReasonConfirmDialogProps<Input>) {
  const toast = useToast();
  const form = useForm({ defaultValues: { reason: "" } });
  const confirm = form.handleSubmit(({ reason }) =>
    command.mutate(
      { input: input(reason.trim()) },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: successTitle });
          onClose();
        },
      },
    ),
  );
  return (
    <ConfirmDialog
      title={title}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={confirmLabel}
      onConfirm={() => void confirm()}
      isPending={command.isPending}
      variant={variant}
    >
      <div className={styles.body}>
        {children}
        <FormProvider {...form}>
          <ReasonField />
        </FormProvider>
        {command.error && <ErrorState title="Not saved" error={command.error} />}
      </div>
    </ConfirmDialog>
  );
}
