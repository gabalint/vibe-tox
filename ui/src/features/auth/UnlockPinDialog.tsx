import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { DisableUnlockPinInput } from "@/generated/DisableUnlockPinInput";
import type { UnlockPinStatus } from "@/generated/UnlockPinStatus";
import { useCommandQuery } from "@/lib/commands";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import styles from "@/styles/layout.module.css";
import { SetUnlockPinDialog } from "./SetUnlockPinDialog";

/** Optional convenience for one's own existing session, never an account sign-in method. */
export function UnlockPinDialog({ onClose }: { onClose: () => void }) {
  const [action, setAction] = useState<"set" | "disable" | null>(null);
  const status = useCommandQuery<UnlockPinStatus>("get_unlock_pin_status");
  if (action === "set" && status.data)
    return <SetUnlockPinDialog enabled={status.data.enabled} onClose={onClose} />;
  if (action === "disable")
    return (
      <ReauthConfirmDialog<Record<string, never>, DisableUnlockPinInput, null>
        title="Disable unlock PIN"
        confirmLabel="Disable PIN"
        variant="primary"
        command={{ name: "disable_unlock_pin", invalidates: ["get_unlock_pin_status"] }}
        extraDefaults={{}}
        toInput={({ password }) => ({ password })}
        successTitle={() => "Unlock PIN disabled"}
        errorTitle="The unlock PIN was not disabled"
        onClose={onClose}
      >
        <p>
          Future unlocks will require your password or registered security key. Your other sessions
          will end; this session stays open.
        </p>
      </ReauthConfirmDialog>
    );
  return (
    <Dialog
      title="Unlock PIN"
      isOpen
      onOpenChange={(open) => !open && onClose()}
      actions={<Button onPress={onClose}>Close</Button>}
    >
      <div className={styles.stack}>
        <p>
          An optional personal PIN lets you unlock your current session. You still sign in after
          restarting the app, signing out or reaching the session’s 12-hour limit. Sensitive actions
          still require your password or security key.
        </p>
        {status.isPending && <LoadingState label="Checking unlock PIN…" />}
        {status.isError && (
          <ErrorState
            title="The unlock PIN status could not be checked"
            error={status.error}
            onRetry={() => void status.refetch()}
          />
        )}
        {status.isSuccess && (
          <>
            <p>
              Unlock PIN is <strong>{status.data.enabled ? "enabled" : "disabled"}</strong>.
            </p>
            <p>
              Five incorrect PIN attempts end the session. You can always choose your normal unlock
              method.
            </p>
            <div className={styles.row}>
              <Button variant="primary" onPress={() => setAction("set")}>
                {status.data.enabled ? "Change PIN" : "Enable PIN"}
              </Button>
              {status.data.enabled && (
                <Button onPress={() => setAction("disable")}>Disable PIN</Button>
              )}
            </div>
          </>
        )}
      </div>
    </Dialog>
  );
}
