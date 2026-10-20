import type { ReactNode } from "react";
import { TextInput } from "@/components/FormFields";
import type { CommandName } from "@/generated/CommandName";
import { CommandDialog } from "@/lib/CommandDialog";

interface NoteDialogProps<Input> {
  title: string;
  submitLabel: string;
  command: { name: CommandName; invalidates: readonly CommandName[] };
  /** The command input for the trimmed note. */
  toInput: (note: string) => Input;
  successTitle: string;
  onClose: () => void;
  /** What the record means, in laboratory terms. */
  children: ReactNode;
}

/** Records one health decision with a required note, which the engine keeps in the audit trail. */
export function NoteDialog<Input>({ toInput, children, ...props }: NoteDialogProps<Input>) {
  return (
    <CommandDialog<{ note: string }, Input>
      {...props}
      defaultValues={{ note: "" }}
      toInput={({ note }) => toInput(note.trim())}
    >
      {children}
      <TextInput
        name="note"
        label="Note"
        description="Recorded in the audit trail."
        isRequired
        autoFocus
      />
    </CommandDialog>
  );
}
