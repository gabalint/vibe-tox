import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { TextInput } from "@/components/FormFields";
import type { BeginRestoreInput } from "@/generated/BeginRestoreInput";
import type { RestoreSessionView } from "@/generated/RestoreSessionView";
import { useCommandMutation } from "@/lib/commands";
import { BEGIN_RESTORE } from "../contract";
import { Step } from "./Step";

interface KeyStepProps {
  onBegun: (session: RestoreSessionView) => void;
  /** Returns to the setup screen; nothing has started yet. */
  onBack: () => void;
}

/**
 * The installation key from the recovery kit. Only its presence is checked here; the engine
 * checks its format and checksum and keeps it in memory for this restore only.
 */
export function KeyStep({ onBegun, onBack }: KeyStepProps) {
  const begin = useCommandMutation<RestoreSessionView, { input: BeginRestoreInput }>(
    BEGIN_RESTORE,
    { invalidates: [] },
  );
  const form = useForm<BeginRestoreInput>({ defaultValues: { installationKey: "" } });
  const onSubmit = form.handleSubmit(({ installationKey }) =>
    begin.mutate({ input: { installationKey: installationKey.trim() } }, { onSuccess: onBegun }),
  );
  return (
    <Step title="Installation key">
      <p>
        Restoring needs the recovery kit of the computer the backups came from. The installation key
        in it opens the backups and the stored sign-in codes.
      </p>
      <FormProvider {...form}>
        <Form
          aria-label="Installation key"
          onSubmit={onSubmit}
          error={begin.error}
          errorTitle="The key was not accepted"
          actions={
            <>
              <Button type="submit" variant="primary" isPending={begin.isPending}>
                Continue
              </Button>
              <Button isDisabled={begin.isPending} onPress={onBack}>
                Back
              </Button>
            </>
          }
        >
          <TextInput
            name="installationKey"
            label="Installation key"
            description="Type it exactly as printed in the recovery kit."
            isRequired
            autoFocus
          />
        </Form>
      </FormProvider>
    </Step>
  );
}
