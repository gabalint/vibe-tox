import type { FormEvent, ReactNode } from "react";
import type { FieldValues, UseFormReturn } from "react-hook-form";
import { Button } from "@/components/Button";
import { FormDialog } from "@/components/FormDialog";
import type { CommandError } from "@/generated/CommandError";

/**
 * Heading of a refused user change. The engine's message says why: for `conflict`, the user
 * changed since the list was loaded, the change would leave no active administrator, or the
 * account already has that status.
 */
export const NOT_SAVED_TITLE = "The change was not saved";

interface UserFormDialogProps<Values extends FieldValues> {
  title: string;
  submitLabel: string;
  form: UseFormReturn<Values>;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  /** The command's state: its last error and whether it is running. */
  command: { error: CommandError | null; isPending: boolean };
  onClose: () => void;
  /**
   * Reloads the list after a `conflict`, which shows the user's current state. Omit it where
   * a conflict does not involve an existing user, such as a username already in use.
   */
  onReload?: () => void;
  children: ReactNode;
}

/** A dialog with one form that changes users, offering a reload after a conflict. */
export function UserFormDialog<Values extends FieldValues>({
  command,
  onReload,
  ...rest
}: UserFormDialogProps<Values>) {
  const conflict = command.error?.code === "conflict" && onReload !== undefined;
  return (
    <FormDialog
      {...rest}
      command={command}
      errorTitle={NOT_SAVED_TITLE}
      extraActions={conflict && <Button onPress={onReload}>Reload users</Button>}
    />
  );
}
