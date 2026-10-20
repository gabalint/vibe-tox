import { useQueryClient } from "@tanstack/react-query";
import { FormProvider, useForm } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import type { SecurityKeyEnrollInput } from "@/generated/SecurityKeyEnrollInput";
import type { SessionInfo } from "@/generated/SessionInfo";
import { useCommandMutation } from "@/lib/commands";
import {
  type AuthenticationStatus,
  type KeyEnrollmentResult,
  securityKeySelection,
} from "@/lib/securityKey";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { KeyNameField } from "./KeyNameField";

export function KeyEnrollmentForm({
  status,
  session,
  onDone,
  onCancel,
}: {
  status: AuthenticationStatus;
  session: SessionInfo;
  onDone: (result: KeyEnrollmentResult) => void;
  onCancel: () => void;
}) {
  const client = useQueryClient();
  const form = useForm<{ keyLabel: string; password: string }>({
    defaultValues: { keyLabel: "", password: "" },
  });
  const enroll = useCommandMutation<KeyEnrollmentResult, { input: SecurityKeyEnrollInput }>(
    "security_key_enroll",
    { invalidates: ["get_authentication_status"] },
  );
  const keyAccount = status.method === "security_key";
  return (
    <FormProvider {...form}>
      <Form
        aria-label={keyAccount ? "Add security key" : "Switch to a security key"}
        error={enroll.error}
        errorTitle="The security key was not registered"
        onSubmit={form.handleSubmit((values) => {
          const selected = client.getQueryData<string>(securityKeySelection(session.userId));
          const credentialId =
            status.credentials.find((key) => key.credentialId === selected)?.credentialId ??
            status.credentials[0]?.credentialId;
          enroll.mutate(
            { input: { ...values, credentialId: keyAccount ? (credentialId ?? null) : null } },
            {
              onSuccess: (result) => {
                enroll.reset();
                onDone(result);
              },
            },
          );
        })}
        actions={
          <>
            <Button type="submit" variant="primary" isPending={enroll.isPending}>
              {keyAccount ? "Register another key" : "Register key and switch sign-in"}
            </Button>
            <Button onPress={onCancel} isDisabled={enroll.isPending}>
              Cancel
            </Button>
          </>
        }
      >
        <p>
          {keyAccount
            ? "Verify with an existing key first, then connect a different physical key to register it. Keep your spare separately. A new recovery code replaces your previous code."
            : "Verify your current password, then register a personal security key. It will replace password and authenticator-app sign-in for this account."}
        </p>
        {status.allowSecurityKeyTouchOnly ? (
          <Alert tone="warning" title="This laboratory allows sign-in without a key PIN">
            This new key may use touch only. A lost or unattended key could let someone sign in as
            you. Keys with a PIN or biometric check may still ask for it. Sensitive actions,
            including adding another key, still need a PIN or biometric check: configure one on this
            key or use another registered key that supports it.
          </Alert>
        ) : (
          <p>
            This laboratory requires the key's PIN or biometric verification. If your key has
            neither, configure a PIN with its management software before registering it.
          </p>
        )}
        <KeyNameField />
        <AuthenticationField />
      </Form>
    </FormProvider>
  );
}
