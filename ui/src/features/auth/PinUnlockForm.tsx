import { useQueryClient } from "@tanstack/react-query";
import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UnlockWithPinInput } from "@/generated/UnlockWithPinInput";
import { CURRENT_SESSION, useCommandMutation } from "@/lib/commands";
import { UnlockPinField } from "./UnlockPinField";

export function PinUnlockForm({ attemptsRemaining }: { attemptsRemaining: number }) {
  const client = useQueryClient();
  const form = useForm<UnlockWithPinInput>({ defaultValues: { pin: "" } });
  const unlock = useCommandMutation<SessionInfo, { input: UnlockWithPinInput }>("unlock_with_pin", {
    invalidates: "all",
  });
  const submit = form.handleSubmit((input) => {
    form.resetField("pin");
    unlock.mutate(
      { input },
      {
        onError: () => {
          form.setFocus("pin");
          void client.invalidateQueries({ queryKey: ["get_unlock_pin_status"] });
          void client.invalidateQueries({ queryKey: [CURRENT_SESSION] });
        },
      },
    );
  });
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Unlock with PIN"
        onSubmit={submit}
        error={unlock.error}
        errorTitle="The session was not unlocked"
        actions={
          <Button type="submit" variant="primary" isPending={unlock.isPending}>
            Unlock
          </Button>
        }
      >
        <UnlockPinField autoFocus />
        <p aria-live="polite">
          {attemptsRemaining} {attemptsRemaining === 1 ? "attempt" : "attempts"} remaining before
          you must sign in again.
        </p>
      </Form>
    </FormProvider>
  );
}
