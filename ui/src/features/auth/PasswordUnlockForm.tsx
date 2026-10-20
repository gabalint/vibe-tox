import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { PasswordField } from "@/components/PasswordField";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UnlockSessionInput } from "@/generated/UnlockSessionInput";
import { useCommandMutation } from "@/lib/commands";

export function PasswordUnlockForm() {
  const unlock = useCommandMutation<SessionInfo, { input: UnlockSessionInput }>("unlock_session", {
    invalidates: "all",
  });
  const form = useForm<UnlockSessionInput>({ defaultValues: { password: "" } });
  const submit = form.handleSubmit((input) =>
    unlock.mutate(
      { input },
      {
        onError: () => {
          form.resetField("password");
          form.setFocus("password");
        },
      },
    ),
  );
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Unlock"
        onSubmit={submit}
        error={unlock.error}
        errorTitle="The session did not change"
        actions={
          <Button type="submit" variant="primary" isPending={unlock.isPending}>
            Unlock
          </Button>
        }
      >
        <PasswordField autoFocus />
      </Form>
    </FormProvider>
  );
}
