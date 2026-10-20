import type { CatalogCodedName } from "@/generated/CatalogCodedName";
import type { CommandName } from "@/generated/CommandName";
import { CommandDialog } from "@/lib/CommandDialog";
import { CodedNameFields, trimCodedName } from "./CodedNameFields";

interface AddCodedDialogProps {
  /** Such as "Method": the dialog is "Add method", the notice "Method SYN-IA added". */
  noun: string;
  /** `create_*` with a code and name, answering the new record's ID. */
  command: CommandName;
  invalidates: readonly CommandName[];
  /** Opens the new record. */
  onSaved: (id: string) => void;
  onClose: () => void;
}

/** Adds a catalog record that needs only a code and a name, then opens it. */
export function AddCodedDialog({
  noun,
  command,
  invalidates,
  onSaved,
  onClose,
}: AddCodedDialogProps) {
  const title = `Add ${noun.toLowerCase()}`;
  return (
    <CommandDialog<CatalogCodedName, CatalogCodedName, string>
      title={title}
      submitLabel={title}
      command={{ name: command, invalidates }}
      defaultValues={{ code: "", name: "" }}
      toInput={trimCodedName}
      successTitle={(values) => `${noun} ${values.code.trim()} added`}
      onDone={onSaved}
      errorTitle="Not saved"
      onClose={onClose}
    >
      <CodedNameFields />
    </CommandDialog>
  );
}
