import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import type { SecurityKeyCredential } from "@/lib/securityKey";

export function RevokeKeyDialog({
  credential,
  onClose,
}: {
  credential: SecurityKeyCredential;
  onClose: () => void;
}) {
  return (
    <ReauthConfirmDialog<Record<string, never>, RevokeSecurityKeyInput, null>
      title="Revoke security key"
      confirmLabel="Revoke key"
      variant="danger"
      command={{ name: "revoke_security_key", invalidates: "all" }}
      extraDefaults={{}}
      withReason
      toInput={({ password, reason }) => ({
        credentialId: credential.credentialId,
        expectedRevision: credential.revision,
        password,
        reason,
      })}
      successTitle={() => "Security key revoked"}
      errorTitle="The security key was not revoked"
      onClose={onClose}
    >
      <p>
        Revoke <strong>{credential.label}</strong> so it can no longer access your account. This
        ends every session for your account, including this one. Keep another registered key
        available to sign in again.
      </p>
    </ReauthConfirmDialog>
  );
}

import type { RevokeSecurityKeyInput } from "@/generated/RevokeSecurityKeyInput";
