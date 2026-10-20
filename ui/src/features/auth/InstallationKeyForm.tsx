import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { TextInput } from "@/components/FormFields";
import type { InstallationKeyStateView } from "@/generated/InstallationKeyStateView";
import type { RestoreInstallationKeyInput } from "@/generated/RestoreInstallationKeyInput";
import { useCommandMutation } from "@/lib/commands";
import { GET_INSTALLATION_KEY_STATE, RESTORE_INSTALLATION_KEY } from "./contract";

/**
 * The installation key from the recovery kit, written to this computer's key store. Only its
 * presence is checked here; the engine checks the key, refuses one that is malformed or does
 * not open this installation's data on the key field, and answers the new key state.
 */
export function InstallationKeyForm({ onRestored }: { onRestored: () => void }) {
  const restore = useCommandMutation<
    InstallationKeyStateView,
    { input: RestoreInstallationKeyInput }
  >(RESTORE_INSTALLATION_KEY, { invalidates: [GET_INSTALLATION_KEY_STATE] });
  const form = useForm<RestoreInstallationKeyInput>({ defaultValues: { installationKey: "" } });
  const onSubmit = form.handleSubmit(({ installationKey }) =>
    restore.mutate(
      { input: { installationKey: installationKey.trim() } },
      {
        onSuccess: ({ keyState }) => {
          if (keyState === "ok") onRestored();
        },
      },
    ),
  );
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Restore the installation key"
        onSubmit={onSubmit}
        error={restore.error}
        errorTitle="The installation key was not restored"
        actions={
          <Button type="submit" isPending={restore.isPending}>
            Restore the key
          </Button>
        }
      >
        <TextInput
          name="installationKey"
          label="Installation key"
          description="As printed in the recovery kit."
          isRequired
        />
      </Form>
    </FormProvider>
  );
}
