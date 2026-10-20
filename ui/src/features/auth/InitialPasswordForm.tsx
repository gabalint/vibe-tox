import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import type { ChangeInitialPasswordInput } from "@/generated/ChangeInitialPasswordInput";
import type { SignInStep } from "@/generated/SignInStep";
import { useCommandMutation } from "@/lib/commands";
import { NewPasswordFields, type NewPasswordValues } from "./NewPasswordFields";

interface InitialPasswordFormProps {
  challengeId: string;
  onStep: (step: SignInStep) => void;
  /** Returns to the username and password step. */
  onCancel: () => void;
  /** Returns to the username and password step after the challenge expired. */
  onExpired: () => void;
}

/** Replaces a temporary password set by an administrator before sign-in continues. */
export function InitialPasswordForm({
  challengeId,
  onStep,
  onCancel,
  onExpired,
}: InitialPasswordFormProps) {
  const change = useCommandMutation<SignInStep, { input: ChangeInitialPasswordInput }>(
    "change_initial_password",
    { invalidates: [] },
  );
  const form = useForm<NewPasswordValues>({
    defaultValues: { newPassword: "", confirmPassword: "" },
  });
  const onSubmit = form.handleSubmit(({ newPassword }) =>
    change.mutate(
      { input: { challengeId, newPassword } },
      {
        onSuccess: onStep,
        onError: (error) => {
          if (error.code === "not_found") onExpired();
        },
      },
    ),
  );

  return (
    <FormProvider {...form}>
      <p>Your password was set by an administrator. Choose your own before you continue.</p>
      <Form
        aria-label="Choose a new password"
        onSubmit={onSubmit}
        error={change.error}
        errorTitle="The password was not changed"
        actions={
          <>
            <Button type="submit" variant="primary" isPending={change.isPending}>
              Change password
            </Button>
            <Button isDisabled={change.isPending} onPress={onCancel}>
              Back
            </Button>
          </>
        }
      >
        <NewPasswordFields />
      </Form>
    </FormProvider>
  );
}
