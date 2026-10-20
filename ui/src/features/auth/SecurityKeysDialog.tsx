import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { SessionInfo } from "@/generated/SessionInfo";
import { useCommandQuery } from "@/lib/commands";
import type { AuthenticationStatus, SecurityKeyCredential } from "@/lib/securityKey";
import layout from "@/styles/layout.module.css";
import { KeyEnrollmentForm } from "./KeyEnrollmentForm";
import { KeyRecoveryCode } from "./KeyRecoveryCode";
import { RevokeKeyDialog } from "./RevokeKeyDialog";

/** Own credentials only; administration of other accounts keeps its separate permission checks. */
export function SecurityKeysDialog({
  session,
  onClose,
}: {
  session: SessionInfo;
  onClose: () => void;
}) {
  const [enrolling, setEnrolling] = useState(false);
  const [recoveryCode, setRecoveryCode] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<SecurityKeyCredential | null>(null);
  return (
    <>
      <Dialog
        title="Security keys"
        isOpen
        isDismissable={!recoveryCode && !enrolling}
        onOpenChange={(open) => {
          if (!open) onClose();
        }}
        actions={!enrolling && !recoveryCode ? <Button onPress={onClose}>Close</Button> : undefined}
      >
        <div className={layout.stack}>
          <p>
            Personal keys for <strong>{session.displayName}</strong> ({session.username}).
          </p>
          <SecurityKeysBody
            session={session}
            enrolling={enrolling}
            recoveryCode={recoveryCode}
            setEnrolling={setEnrolling}
            setRecoveryCode={setRecoveryCode}
            setRevoking={setRevoking}
          />
        </div>
      </Dialog>
      {revoking && <RevokeKeyDialog credential={revoking} onClose={() => setRevoking(null)} />}
    </>
  );
}

interface BodyProps {
  session: SessionInfo;
  enrolling: boolean;
  recoveryCode: string | null;
  setEnrolling: (value: boolean) => void;
  setRecoveryCode: (value: string | null) => void;
  setRevoking: (value: SecurityKeyCredential) => void;
}

function SecurityKeysBody({
  session,
  enrolling,
  recoveryCode,
  setEnrolling,
  setRecoveryCode,
  setRevoking,
}: BodyProps) {
  const status = useCommandQuery<AuthenticationStatus>("get_authentication_status");
  if (recoveryCode)
    return <KeyRecoveryCode code={recoveryCode} onSaved={() => setRecoveryCode(null)} />;
  if (status.isPending) return <LoadingState label="Loading security keys…" />;
  if (status.isError)
    return (
      <ErrorState
        title="Security keys could not be loaded"
        error={status.error}
        onRetry={() => void status.refetch()}
      />
    );
  if (enrolling)
    return (
      <KeyEnrollmentForm
        status={status.data}
        session={session}
        onCancel={() => setEnrolling(false)}
        onDone={(result) => {
          setEnrolling(false);
          setRecoveryCode(result.recoveryCode);
        }}
      />
    );
  const keyAccount = status.data.method === "security_key";
  return (
    <>
      <p>
        {keyAccount
          ? "Use a registered key for sign-in and unlock. Sensitive actions always require the key's PIN or biometric verification."
          : "You currently sign in with a password and authenticator app."}
      </p>
      {status.data.credentials.map((credential) => (
        <div className={layout.row} key={credential.credentialId}>
          <span>{credential.label}</span>
          <Button variant="quiet" onPress={() => setRevoking(credential)}>
            Revoke {credential.label}
          </Button>
        </div>
      ))}
      <Button onPress={() => setEnrolling(true)}>
        {keyAccount ? "Add security key" : "Switch to a security key"}
      </Button>
      {keyAccount && (
        <p>
          {status.data.recoveryAvailable
            ? "A recovery code is available. Keep the saved copy secure; it cannot be displayed again."
            : "No recovery code is available. Keep a registered spare key accessible."}
        </p>
      )}
    </>
  );
}
