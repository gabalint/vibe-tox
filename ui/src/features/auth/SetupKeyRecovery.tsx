import { useQueryClient } from "@tanstack/react-query";
import { Alert } from "@/components/Alert";
import type { KeyRecoveryAcknowledgement } from "@/generated/KeyRecoveryAcknowledgement";
import type { SessionInfo } from "@/generated/SessionInfo";
import { useCommandMutation } from "@/lib/commands";
import { SETUP_STATUS } from "./contract";
import { KeyRecoveryCode } from "./KeyRecoveryCode";

/** The account exists already; a spent setup permit falls back to fresh key sign-in. */
export function SetupKeyRecovery({ code }: { code: string }) {
  const client = useQueryClient();
  const acknowledge = useCommandMutation<SessionInfo, { input: KeyRecoveryAcknowledgement }>(
    "acknowledge_key_recovery",
    { invalidates: "all" },
  );
  return (
    <>
      {acknowledge.isError && (
        <Alert tone="info">
          Your account was created. Save the code, then continue to sign in with your registered
          key.
        </Alert>
      )}
      <KeyRecoveryCode
        code={code}
        pending={acknowledge.isPending}
        error={acknowledge.error}
        continueLabel={acknowledge.isError ? "Continue to sign in" : "Continue"}
        onSaved={() =>
          acknowledge.isError
            ? void client.invalidateQueries({ queryKey: [SETUP_STATUS] })
            : acknowledge.mutate({ input: { recoveryCode: code } })
        }
      />
    </>
  );
}
