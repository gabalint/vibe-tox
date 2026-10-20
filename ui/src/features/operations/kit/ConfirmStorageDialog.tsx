import { TextInput } from "@/components/FormFields";
import type { ConfirmRecoveryKitStorageInput } from "@/generated/ConfirmRecoveryKitStorageInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { CONFIRM_RECOVERY_KIT_STORAGE, KIT_QUERIES } from "../contract";

interface ConfirmStorageDialogProps {
  kitNo: number;
  onClose: () => void;
}

/** Records that the latest kit is stored where the laboratory keeps it (design brief §2.4). */
export function ConfirmStorageDialog({ kitNo, onClose }: ConfirmStorageDialogProps) {
  return (
    <CommandDialog<{ note: string }, ConfirmRecoveryKitStorageInput>
      title={`Confirm where kit ${kitNo} is stored`}
      submitLabel="Confirm storage"
      command={{ name: CONFIRM_RECOVERY_KIT_STORAGE, invalidates: KIT_QUERIES }}
      defaultValues={{ note: "" }}
      toInput={({ note }) => ({ kitNo, note: note.trim() })}
      successTitle={`Storage of kit ${kitNo} confirmed`}
      onClose={onClose}
    >
      <TextInput
        name="note"
        label="Where the kit is stored"
        description="Name both places off this computer. Recorded in the audit trail."
        isRequired
      />
    </CommandDialog>
  );
}
