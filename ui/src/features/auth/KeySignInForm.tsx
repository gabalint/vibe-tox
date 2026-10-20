import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Form } from "@/components/Form";
import { TextInput } from "@/components/FormFields/TextInput";
import { LoadingState } from "@/components/LoadingState";
import { Select } from "@/components/Select";
import type { SecurityKeySignInInput } from "@/generated/SecurityKeySignInInput";
import type { SecurityKeyUnlockInput } from "@/generated/SecurityKeyUnlockInput";
import type { SessionInfo } from "@/generated/SessionInfo";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import type { SecurityKeyCredential } from "@/lib/securityKey";

/** Usernames are looked up only after submission, not on each keystroke. */
export function KeySignInForm({
  username,
  locked = false,
}: {
  username?: string;
  locked?: boolean;
}) {
  const [chosenUsername, setUsername] = useState(username ?? "");
  const form = useForm<{ username: string }>({ defaultValues: { username: "" } });
  if (chosenUsername)
    return (
      <SelectedKeySignIn
        username={chosenUsername}
        locked={locked}
        onBack={username ? undefined : () => setUsername("")}
      />
    );
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Find your security key"
        onSubmit={form.handleSubmit((values) => setUsername(values.username.trim()))}
        actions={
          <Button type="submit" variant="primary">
            Find my keys
          </Button>
        }
      >
        <TextInput name="username" label="Username" isRequired autoFocus />
      </Form>
    </FormProvider>
  );
}

function SelectedKeySignIn({
  username,
  locked,
  onBack,
}: {
  username: string;
  locked: boolean;
  onBack?: () => void;
}) {
  const keys = useCommandQuery<SecurityKeyCredential[]>("list_key_sign_in_choices", {
    input: { username },
  });
  const [selected, setSelected] = useState<string | null>(null);
  const signIn = useCommandMutation<
    SessionInfo,
    { input: SecurityKeySignInInput | SecurityKeyUnlockInput }
  >(locked ? "security_key_unlock" : "security_key_sign_in", { invalidates: "all" });
  if (keys.isPending) return <LoadingState label="Finding your security keys…" />;
  if (keys.isError)
    return (
      <>
        <ErrorState
          title="Security keys could not be listed"
          error={keys.error}
          onRetry={() => void keys.refetch()}
        />
        {onBack && <Button onPress={onBack}>Use another username</Button>}
      </>
    );
  const credentialId = keys.data.some((key) => key.credentialId === selected)
    ? selected
    : keys.data[0]?.credentialId;
  return (
    <Form
      aria-label={locked ? "Unlock with security key" : "Sign in with security key"}
      onSubmit={(event) => {
        event.preventDefault();
        if (credentialId)
          signIn.mutate({ input: { credentialId, ...(locked ? {} : { username }) } });
      }}
      error={signIn.error}
      errorTitle={locked ? "The session was not unlocked" : "Sign-in failed"}
      actions={
        <>
          <Button
            type="submit"
            variant="primary"
            isPending={signIn.isPending}
            isDisabled={!credentialId}
          >
            {locked ? "Unlock with security key" : "Sign in with security key"}
          </Button>
          {onBack && (
            <Button onPress={onBack} isDisabled={signIn.isPending}>
              Use another username
            </Button>
          )}
        </>
      }
    >
      <p>
        Signing in as <strong>{username}</strong>.
      </p>
      {keys.data.length === 0 ? (
        <p>
          No registered key is available for this username. Check the username or use your recovery
          code.
        </p>
      ) : (
        <Select
          name="credentialId"
          label="Security key"
          value={credentialId ?? null}
          onChange={setSelected}
          options={keys.data.map((key) => ({ id: key.credentialId, label: key.label }))}
          isRequired
          isDisabled={signIn.isPending}
        />
      )}
    </Form>
  );
}
