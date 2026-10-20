import { useState } from "react";
import { Alert } from "@/components/Alert";
import type { InstallationKeyStateView } from "@/generated/InstallationKeyStateView";
import { useCommandQuery } from "@/lib/commands";
import styles from "./auth.module.css";
import { GET_INSTALLATION_KEY_STATE } from "./contract";
import { InstallationKeyForm } from "./InstallationKeyForm";

const KEY_PROBLEMS = {
  missing: "This computer does not hold the installation key that protects sign-in codes.",
  mismatch:
    "The installation key on this computer does not match the one that protects the stored sign-in codes.",
} as const;

/**
 * On the sign-in screen: what the engine reports about the installation key (M4 design brief
 * §3.9), with a way to restore it from the recovery kit. Says nothing while the key is fine or
 * its state cannot be read; signing in is offered either way.
 */
export function InstallationKeyPanel() {
  const key = useCommandQuery<InstallationKeyStateView>(GET_INSTALLATION_KEY_STATE);
  const [restored, setRestored] = useState(false);
  if (!key.isSuccess) return null;
  const { keyState } = key.data;
  if (keyState === "unavailable") {
    return (
      <Alert tone="warning" title="The key store could not be read">
        The operating system's key store could not be read, so this computer cannot check sign-in
        codes. Ask the person who looks after this computer.
      </Alert>
    );
  }
  if (keyState === "ok") {
    return restored ? (
      <Alert tone="success">
        The installation key was restored. This computer can check sign-in codes again.
      </Alert>
    ) : null;
  }
  return (
    <div className={styles.flow}>
      <Alert tone="warning" title="Installation key needed">
        {KEY_PROBLEMS[keyState]} Enter the installation key from the recovery kit.
      </Alert>
      <InstallationKeyForm onRestored={() => setRestored(true)} />
    </div>
  );
}
