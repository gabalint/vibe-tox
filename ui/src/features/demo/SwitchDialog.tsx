import type { ReactNode } from "react";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";

interface SwitchDialogProps {
  title: string;
  confirmLabel: string;
  variant: "primary" | "danger";
  /** The shell command that records the switch and restarts the app. */
  command: CommandName;
  onClose: () => void;
  /** What will happen, in laboratory terms. */
  children: ReactNode;
}

/**
 * Confirms a switch between the laboratory and the demo laboratory. On success the app
 * restarts, so the dialog stays open, pending, until the window closes; a refusal, such as
 * work still in progress, stays in the dialog.
 */
export function SwitchDialog({
  title,
  confirmLabel,
  variant,
  command,
  onClose,
  children,
}: SwitchDialogProps) {
  const run = useCommandMutation<null>(command, { invalidates: [] });
  return (
    <ConfirmDialog
      title={title}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={confirmLabel}
      variant={variant}
      isPending={run.isPending || run.isSuccess}
      onConfirm={() => run.mutate({})}
    >
      <div className={styles.stack}>
        {children}
        {run.isSuccess && <p role="status">Restarting…</p>}
        {run.error && <ErrorState title="The app did not switch" error={run.error} />}
      </div>
    </ConfirmDialog>
  );
}
