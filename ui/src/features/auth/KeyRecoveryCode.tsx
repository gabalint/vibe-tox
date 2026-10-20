import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { ErrorState } from "@/components/ErrorState";
import type { CommandError } from "@/generated/CommandError";
import styles from "./auth.module.css";

/** The code is shown once; acknowledgment never stores the secret in browser storage. */
export function KeyRecoveryCode({
  code,
  onSaved,
  pending = false,
  error,
  continueLabel = "Continue",
}: {
  code: string;
  onSaved: () => void;
  pending?: boolean;
  error?: CommandError | null;
  continueLabel?: string;
}) {
  const [saved, setSaved] = useState(false);
  return (
    <div className={styles.flow}>
      <h2 className={styles.stepTitle}>Save your recovery code</h2>
      <p>
        Keep this code in a secure place away from this computer. It lets you register a replacement
        key if you lose access to every key.
      </p>
      <Alert tone="warning">
        Anyone with this code can replace your sign-in key. It will not be shown again.
      </Alert>
      <div className={styles.secret}>
        <strong>Recovery code</strong>
        <code className={styles.key}>{code}</code>
      </div>
      <Checkbox isSelected={saved} onChange={setSaved} isDisabled={pending}>
        I saved this code away from this computer
      </Checkbox>
      {error && <ErrorState title="Recovery-code confirmation failed" error={error} />}
      <Button variant="primary" isDisabled={!saved} isPending={pending} onPress={onSaved}>
        {continueLabel}
      </Button>
    </div>
  );
}
