import { useState } from "react";
import { useForm } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { useToast } from "@/components/Toast";
import type { AddBackupTargetInput } from "@/generated/AddBackupTargetInput";
import type { TargetKind } from "@/generated/TargetKind";
import type { TargetPurpose } from "@/generated/TargetPurpose";
import type { TargetView } from "@/generated/TargetView";
import { checked } from "@/lib/choices";
import { useCommandMutation } from "@/lib/commands";
import { ADD_BACKUP_TARGET, BACKUP_QUERIES } from "../contract";
import { TARGET_KINDS, TARGET_PURPOSES } from "../labels";

interface TargetValues {
  label: string;
  kind: TargetKind | null;
  purpose: TargetPurpose | null;
}

/**
 * Adds a backup target. The app then opens a folder dialog; the engine checks the folder and
 * marks it as this installation's. Cancelling the folder dialog adds nothing, and this dialog
 * stays open so the folder can be chosen again.
 */
export function AddTargetDialog({ onClose }: { onClose: () => void }) {
  const toast = useToast();
  const [cancelled, setCancelled] = useState(false);
  const add = useCommandMutation<TargetView | null, { input: AddBackupTargetInput }>(
    ADD_BACKUP_TARGET,
    {
      invalidates: BACKUP_QUERIES,
    },
  );
  const form = useForm<TargetValues>({ defaultValues: { label: "", kind: null, purpose: null } });
  const submit = form.handleSubmit((values) => {
    setCancelled(false);
    const input = {
      label: values.label.trim(),
      kind: checked(values.kind),
      purpose: checked(values.purpose),
    };
    add.mutate(
      { input },
      {
        onSuccess: (target) => {
          if (target === null) {
            setCancelled(true);
            return;
          }
          toast.show({ tone: "success", title: `${target.label} added as a backup target` });
          onClose();
        },
      },
    );
  });
  return (
    <FormDialog
      form={form}
      title="Add backup target"
      submitLabel="Choose folder"
      errorTitle="Target not added"
      command={add}
      onSubmit={submit}
      onClose={onClose}
    >
      <p>
        After you choose the folder, the app writes a small marker file there. Backups are encrypted
        with this installation's key, so the recovery kit is needed to read them.
      </p>
      <TextInput
        name="label"
        label="Name"
        description="How the target is shown, such as “Front office NAS”. No patient information."
        isRequired
        autoFocus
      />
      <ChoiceInput name="kind" label="Kind" options={TARGET_KINDS} isRequired />
      <ChoiceInput name="purpose" label="Receives" options={TARGET_PURPOSES} isRequired />
      {cancelled && (
        <Alert tone="info">The folder dialog was cancelled. No target was added.</Alert>
      )}
    </FormDialog>
  );
}
