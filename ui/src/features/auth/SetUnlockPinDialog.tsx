import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { useToast } from "@/components/Toast";
import type { SetUnlockPinInput } from "@/generated/SetUnlockPinInput";
import { useCommandMutation } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { UnlockPinField } from "./UnlockPinField";

export function SetUnlockPinDialog({
  enabled,
  onClose,
}: {
  enabled: boolean;
  onClose: () => void;
}) {
  const toast = useToast();
  const save = useCommandMutation<null, { input: SetUnlockPinInput }>("set_unlock_pin", {
    invalidates: ["get_unlock_pin_status"],
  });
  const form = useForm<{ pin: string; confirmPin: string; password: string }>({
    defaultValues: { pin: "", confirmPin: "", password: "" },
  });
  const submit = form.handleSubmit(({ pin, password }) => {
    form.resetField("pin");
    form.resetField("confirmPin");
    form.resetField("password");
    save.mutate(
      { input: { pin, password } },
      {
        onSuccess: () => {
          toast.show({
            tone: "success",
            title: enabled ? "Unlock PIN changed" : "Unlock PIN enabled",
          });
          onClose();
        },
      },
    );
  });
  return (
    <FormDialog
      title={enabled ? "Change unlock PIN" : "Enable unlock PIN"}
      submitLabel={enabled ? "Change PIN" : "Enable PIN"}
      errorTitle="The unlock PIN was not saved"
      form={form}
      command={save}
      onSubmit={submit}
      onClose={onClose}
    >
      <p>
        Choose a personal six-digit PIN. Avoid repeated digits, sequences and other predictable
        patterns. Confirm with your password or registered security key.
      </p>
      <p>Saving ends your other sessions; this session stays open.</p>
      <UnlockPinField label="New unlock PIN" autoFocus />
      <UnlockPinField name="confirmPin" label="Confirm unlock PIN" confirm />
      <AuthenticationField />
    </FormDialog>
  );
}
