import { useIsMutating } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UnlockPinStatus } from "@/generated/UnlockPinStatus";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import type { AuthenticationStatus } from "@/lib/securityKey";
import styles from "./auth.module.css";
import { KeySignInForm } from "./KeySignInForm";
import { PasswordUnlockForm } from "./PasswordUnlockForm";
import { PinUnlockForm } from "./PinUnlockForm";

/** Same-person session unlock (REQ-IDN-004); every successful transition clears cached data. */
export function LockScreen({ session }: { session: SessionInfo }) {
  const authentication = useCommandQuery<AuthenticationStatus>("get_authentication_status");
  const pin = useCommandQuery<UnlockPinStatus>("get_unlock_pin_status");
  const signOut = useCommandMutation<null>("sign_out", { invalidates: "all" });
  const pending = useIsMutating() > 0;
  const [useNormalMethod, setUseNormalMethod] = useState(false);
  const usePin = pin.isSuccess && pin.data.enabled && !useNormalMethod;
  const keyAccount = authentication.data?.method === "security_key";
  const normalLabel = authentication.isError
    ? "Use normal unlock instead"
    : keyAccount
      ? "Use security key instead"
      : "Use password instead";
  return (
    <div className={styles.flow}>
      <h1 className={styles.title}>Session locked</h1>
      <p>
        This session of <strong>{session.displayName}</strong> ({session.username}) is locked.
      </p>
      {authentication.isPending || pin.isPending ? (
        <LoadingState label="Checking unlock options…" />
      ) : usePin ? (
        <PinUnlockForm attemptsRemaining={pin.data.attemptsRemaining} />
      ) : (
        <NormalUnlockForm username={session.username} authentication={authentication} />
      )}
      {pin.isError && (
        <ErrorState
          title="The unlock PIN status could not be checked"
          error={pin.error}
          onRetry={() => void pin.refetch()}
        />
      )}
      {pin.isSuccess && pin.data.enabled && (
        <Button isDisabled={pending} onPress={() => setUseNormalMethod(!useNormalMethod)}>
          {usePin ? normalLabel : "Use unlock PIN"}
        </Button>
      )}
      <Button isDisabled={pending} isPending={signOut.isPending} onPress={() => signOut.mutate({})}>
        Sign in as a different user
      </Button>
      {signOut.error && <ErrorState title="The session did not change" error={signOut.error} />}
    </div>
  );
}

function NormalUnlockForm({
  username,
  authentication,
}: {
  username: string;
  authentication: ReturnType<typeof useCommandQuery<AuthenticationStatus>>;
}) {
  if (authentication.isError)
    return (
      <ErrorState
        title="The sign-in method could not be checked"
        error={authentication.error}
        onRetry={() => void authentication.refetch()}
      />
    );
  return authentication.data?.method === "security_key" ? (
    <KeySignInForm username={username} locked />
  ) : (
    <PasswordUnlockForm />
  );
}
