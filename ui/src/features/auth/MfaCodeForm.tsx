import { Controller, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { TextField } from "@/components/TextField";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { VerifyMfaInput } from "@/generated/VerifyMfaInput";
import { useCommandMutation } from "@/lib/commands";
import { textFieldProps } from "@/lib/form";

interface MfaCodeFormProps {
  challengeId: string;
  /** Returns to the username and password step. */
  onCancel: () => void;
  /** Returns to the username and password step after the challenge expired. */
  onExpired: () => void;
}

/**
 * The authenticator code, which works offline (REQ-IDN-004). It completes sign-in, and for
 * an enrollment challenge also confirms the new authenticator. A new session starts with
 * an empty engine cache.
 */
export function MfaCodeForm({ challengeId, onCancel, onExpired }: MfaCodeFormProps) {
  const verify = useCommandMutation<SessionInfo, { input: VerifyMfaInput }>("verify_mfa", {
    invalidates: "all",
  });
  const { control, handleSubmit } = useForm<{ code: string }>({ defaultValues: { code: "" } });
  const onSubmit = handleSubmit(({ code }) =>
    verify.mutate(
      { input: { challengeId, code } },
      {
        onError: (error) => {
          if (error.code === "not_found") onExpired();
        },
      },
    ),
  );

  return (
    <Form
      aria-label="Verification code"
      onSubmit={onSubmit}
      error={verify.error}
      errorTitle="Verification failed"
      actions={
        <>
          <Button type="submit" variant="primary" isPending={verify.isPending}>
            Verify
          </Button>
          <Button isDisabled={verify.isPending} onPress={onCancel}>
            Back
          </Button>
        </>
      }
    >
      <Controller
        control={control}
        name="code"
        rules={{ required: "Enter the code." }}
        render={(controller) => (
          <TextField
            {...textFieldProps(controller)}
            label="Verification code"
            description="The code shown in your authenticator app."
            inputMode="numeric"
            autoComplete="one-time-code"
            isRequired
            autoFocus
          />
        )}
      />
    </Form>
  );
}
