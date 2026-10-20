import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Facts } from "@/components/Facts";
import type { RestoreSessionView } from "@/generated/RestoreSessionView";
import type { StagedRestoreView } from "@/generated/StagedRestoreView";
import layout from "@/styles/layout.module.css";
import { KeyStep } from "./KeyStep";
import { ReviewStep } from "./ReviewStep";
import styles from "./restore.module.css";
import { SourceStep } from "./SourceStep";
import { Step } from "./Step";

type WizardState =
  | { step: "key" }
  | { step: "source"; session: RestoreSessionView }
  | { step: "review"; session: RestoreSessionView; staged: StagedRestoreView }
  | { step: "closing" };

/**
 * Restores the laboratory's data from a backup onto a new installation (REQ-OPS-002; M4 design
 * brief §3.7, D4): the installation key from the recovery kit, the backup folder, a full check
 * of one backup, then the confirmation. The engine stages the backup now and applies it at the
 * next start; the current data folder is kept aside. Every decision is the engine's.
 */
export function RestoreWizard({ onExit }: { onExit: () => void }) {
  const [state, setState] = useState<WizardState>({ step: "key" });
  const session = state.step === "source" || state.step === "review" ? state.session : null;
  return (
    <div className={layout.stack}>
      <h1 className={styles.title}>Restore from a backup</h1>
      {session && (
        <div>
          <Facts
            items={[
              {
                label: "Installation key fingerprint",
                value: <span className={layout.code}>{session.keyFingerprint}</span>,
              },
            ]}
          />
          <p className={layout.hint}>Compare it with the fingerprint in the recovery kit.</p>
        </div>
      )}
      {state.step === "key" && (
        <KeyStep
          onBegun={(begun) => setState({ step: "source", session: begun })}
          onBack={onExit}
        />
      )}
      {state.step === "source" && (
        <SourceStep
          session={state.session}
          onStaged={(staged) => setState({ step: "review", session: state.session, staged })}
          onExit={onExit}
        />
      )}
      {state.step === "review" && (
        <ReviewStep
          session={state.session}
          staged={state.staged}
          onClosing={() => setState({ step: "closing" })}
          onExit={onExit}
        />
      )}
      {state.step === "closing" && (
        <Step title="Restore prepared">
          <Alert tone="info" title="The app is closing">
            The current data folder is kept aside; the app closes; open it again to finish.
          </Alert>
        </Step>
      )}
    </div>
  );
}
