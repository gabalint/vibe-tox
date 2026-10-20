import { type ReactNode, useState } from "react";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { asksSelfApproval, SelfApprovalConfirm } from "@/components/SelfApprovalConfirm";
import { useToast } from "@/components/Toast";
import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";

/** The command's input, or how to build it from a self-approval confirmation. */
type InputSource<Input> =
  | { input: Input; selfApprovalInput?: undefined }
  | {
      input?: undefined;
      /**
       * For the second step of a duty one person may do alone under the laboratory's
       * separation-of-duties policy (ADR 0015): the input, given whether the person confirmed
       * doing it alone. `false` until the engine asks and the person confirms.
       */
      selfApprovalInput: (selfApproval: boolean) => Input;
    };

type CommandConfirmDialogProps<Input, Result> = InputSource<Input> & {
  title: string;
  confirmLabel: string;
  variant: "primary" | "danger";
  command: { name: CommandName; invalidates: readonly CommandName[] };
  successTitle: string;
  onDone?: (result: Result) => void;
  onClose: () => void;
  /** What will happen, in laboratory terms. */
  children: ReactNode;
};

/**
 * Confirms one action that needs no further input, runs its command and closes once the
 * engine has recorded it. A refusal stays in the dialog with its gate reasons; a request to
 * confirm a self-approval shows that confirmation instead. In `lib/` because it calls the engine.
 */
export function CommandConfirmDialog<Input, Result = unknown>(
  props: CommandConfirmDialogProps<Input, Result>,
) {
  const { title, confirmLabel, variant, command, successTitle, onDone, onClose, children } = props;
  const toast = useToast();
  const [selfApproval, setSelfApproval] = useState(false);
  const run = useCommandMutation<Result, { input: Input }>(command.name, {
    invalidates: command.invalidates,
  });
  const confirm = () =>
    run.mutate(
      { input: props.selfApprovalInput ? props.selfApprovalInput(selfApproval) : props.input },
      {
        onSuccess: (result) => {
          toast.show({ tone: "success", title: successTitle });
          onDone?.(result);
          onClose();
        },
      },
    );
  return (
    <ConfirmDialog
      title={title}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={confirmLabel}
      onConfirm={confirm}
      isPending={run.isPending}
      variant={variant}
    >
      <div className={styles.stack}>
        {children}
        <SelfApprovalConfirm
          error={run.error}
          isConfirmed={selfApproval}
          onChange={setSelfApproval}
        />
        {run.error && !asksSelfApproval(run.error) && (
          <ErrorState title="Not recorded" error={run.error} />
        )}
      </div>
    </ConfirmDialog>
  );
}
