import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useFormContext } from "react-hook-form";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { PasswordField } from "@/components/PasswordField";
import { Select } from "@/components/Select";
import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "./commands";
import { type AuthenticationStatus, securityKeySelection } from "./securityKey";

/** Each action keeps its own confirmation; selection alone never authorizes it. */
export function AuthenticationField({ label = "Your password" }: { label?: string }) {
  const status = useCommandQuery<AuthenticationStatus>("get_authentication_status");
  const { setValue, clearErrors, unregister } = useFormContext();
  const isKey = status.data?.method === "security_key";
  useEffect(() => {
    if (isKey) {
      unregister("password");
      setValue("password", "");
      clearErrors("password");
    }
  }, [isKey, setValue, clearErrors, unregister]);

  if (status.isPending) return <LoadingState label="Checking your sign-in method…" />;
  if (status.isError)
    return (
      <ErrorState
        title="Your sign-in method could not be checked"
        error={status.error}
        onRetry={() => void status.refetch()}
      />
    );
  return isKey ? <KeyConfirmationField status={status.data} /> : <PasswordField label={label} />;
}

function KeyConfirmationField({ status }: { status: AuthenticationStatus }) {
  const client = useQueryClient();
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const userId = session.data?.userId ?? "";
  const selection = useQuery<string | null>({
    queryKey: securityKeySelection(userId),
    queryFn: () => null,
    enabled: false,
  });
  const selected = status.credentials.some((key) => key.credentialId === selection.data)
    ? selection.data
    : status.credentials[0]?.credentialId;
  return (
    <>
      <Select
        label="Security key"
        name="securityKey"
        options={status.credentials.map((key) => ({ id: key.credentialId, label: key.label }))}
        value={selected ?? null}
        onChange={(value) => client.setQueryData(securityKeySelection(userId), value)}
        isRequired
        description="Confirm the action with this key. Enter its PIN or use its biometric verification when prompted."
      />
      {status.credentials.length === 0 && (
        <p>No active key is registered. Recover your sign-in first.</p>
      )}
    </>
  );
}
