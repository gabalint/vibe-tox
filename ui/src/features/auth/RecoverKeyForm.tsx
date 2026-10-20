import { useState } from "react";
import { Controller, FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { TextInput } from "@/components/FormFields/TextInput";
import { TextField } from "@/components/TextField";
import type { KeyRecoveryResult } from "@/generated/KeyRecoveryResult";
import type { SecurityKeyRecoveryInput } from "@/generated/SecurityKeyRecoveryInput";
import { useCommandMutation } from "@/lib/commands";
import { textFieldProps } from "@/lib/form";
import { KeyNameField } from "./KeyNameField";
import { KeyRecoveryCode } from "./KeyRecoveryCode";

/** Offline recovery replaces credentials; it never grants a clinical role or releases a hold. */
export function RecoverKeyForm({ onClose }: { onClose: () => void }) {
  const [code, setCode] = useState<string | null>(null);
  const form = useForm<SecurityKeyRecoveryInput>({
    defaultValues: { username: "", recoveryCode: "", keyLabel: "" },
  });
  const recover = useCommandMutation<KeyRecoveryResult, { input: SecurityKeyRecoveryInput }>(
    "security_key_recover",
    { invalidates: [] },
  );
  if (code) return <KeyRecoveryCode code={code} onSaved={onClose} />;
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Recover security key"
        error={recover.error}
        errorTitle="Your key was not replaced"
        onSubmit={form.handleSubmit((input) => {
          form.resetField("recoveryCode");
          recover.mutate(
            { input },
            {
              onSuccess: (result) => {
                setCode(result.recoveryCode);
                recover.reset();
              },
            },
          );
        })}
        actions={
          <>
            <Button type="submit" variant="primary" isPending={recover.isPending}>
              Register replacement key
            </Button>
            <Button onPress={onClose} isDisabled={recover.isPending}>
              Back to sign in
            </Button>
          </>
        }
      >
        <p>
          Use the recovery code you saved when registering your key. Existing keys are revoked and a
          new recovery code replaces this one. Your account permissions stay the same.
        </p>
        <p>
          Recovery requires the replacement key's PIN or biometric verification, even when the
          laboratory allows touch-only sign-in.
        </p>
        <TextInput name="username" label="Username" isRequired autoFocus />
        <Controller
          control={form.control}
          name="recoveryCode"
          rules={{ required: "Enter your saved recovery code." }}
          render={(controller) => (
            <TextField
              {...textFieldProps(controller)}
              label="Recovery code"
              type="password"
              autoComplete="off"
              isRequired
            />
          )}
        />
        <KeyNameField />
      </Form>
    </FormProvider>
  );
}
