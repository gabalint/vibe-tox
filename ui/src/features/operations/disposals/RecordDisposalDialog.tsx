import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { DisposalMethod } from "@/generated/DisposalMethod";
import type { RecordMediaDisposalInput } from "@/generated/RecordMediaDisposalInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, optionalText } from "@/lib/choices";
import { DISPOSAL_QUERIES, RECORD_MEDIA_DISPOSAL } from "../contract";
import { DISPOSAL_METHODS } from "../labels";

interface DisposalValues {
  medium: string;
  method: DisposalMethod | null;
  identifier: string;
  note: string;
}

/** Records how one disk or backup medium was disposed of (REQ-OPS-003). */
export function RecordDisposalDialog({ onClose }: { onClose: () => void }) {
  return (
    <CommandDialog<DisposalValues, RecordMediaDisposalInput>
      title="Record disposal"
      submitLabel="Record disposal"
      command={{ name: RECORD_MEDIA_DISPOSAL, invalidates: DISPOSAL_QUERIES }}
      defaultValues={{ medium: "", method: null, identifier: "", note: "" }}
      toInput={(values) => ({
        medium: values.medium.trim(),
        method: checked(choiceOf(DISPOSAL_METHODS, values.method)),
        identifier: optionalText(values.identifier),
        note: values.note.trim(),
      })}
      successTitle="Disposal recorded"
      onClose={onClose}
    >
      <TextInput
        name="medium"
        label="Medium"
        description="Such as the old host's internal disk or removable backup drive 2."
        isRequired
      />
      <ChoiceInput name="method" label="Method" options={DISPOSAL_METHODS} isRequired />
      <TextInput
        name="identifier"
        label="Identifier"
        description="Such as the serial number or asset tag, if it has one."
      />
      <TextInput
        name="note"
        label="Note"
        description="Such as who did it and any certificate of destruction."
        isRequired
      />
    </CommandDialog>
  );
}
