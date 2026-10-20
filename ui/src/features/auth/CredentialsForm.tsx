import { Controller, FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { PasswordField } from "@/components/PasswordField";
import { TextField } from "@/components/TextField";
import type { SignInInput } from "@/generated/SignInInput";
import type { SignInStep } from "@/generated/SignInStep";
import { useCommandMutation } from "@/lib/commands";
import { textFieldProps } from "@/lib/form";

/**
 * Username and password, the first sign-in step. The engine's message is shown as given:
 * it is generic for wrong credentials and names a lockout when one applies.
 */
export function CredentialsForm({ onStep }: { onStep: (step: SignInStep) => void }) {
  const signIn = useCommandMutation<SignInStep, { input: SignInInput }>("sign_in", {
    invalidates: [],
  });
  const form = useForm<SignInInput>({ defaultValues: { username: "", password: "" } });
  const onSubmit = form.handleSubmit((input) =>
    signIn.mutate(
      { input },
      {
        onSuccess: onStep,
        // Coordinator decision: a failed sign-in clears the password, never the username.
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
        aria-label="Sign in"
        onSubmit={onSubmit}
        error={signIn.error}
        errorTitle="Sign-in failed"
        actions={
          <Button type="submit" variant="primary" isPending={signIn.isPending}>
            Sign in
          </Button>
        }
      >
        <Controller
          control={form.control}
          name="username"
          rules={{ required: "Enter your username." }}
          render={(controller) => (
            <TextField
              {...textFieldProps(controller)}
              label="Username"
              autoComplete="username"
              isRequired
              autoFocus
            />
          )}
        />
        <PasswordField />
      </Form>
    </FormProvider>
  );
}
