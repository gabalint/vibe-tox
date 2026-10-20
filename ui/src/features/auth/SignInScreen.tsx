import { type ReactNode, useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import type { SignInStep } from "@/generated/SignInStep";
import styles from "./auth.module.css";
import { CredentialsForm } from "./CredentialsForm";
import { InitialPasswordForm } from "./InitialPasswordForm";
import { InstallationKeyPanel } from "./InstallationKeyPanel";
import { KeySignInForm } from "./KeySignInForm";
import { MfaCodeForm } from "./MfaCodeForm";
import { MfaEnrollment } from "./MfaEnrollment";
import { RecoverKeyForm } from "./RecoverKeyForm";

const STEP_TITLES: Record<SignInStep["kind"], string> = {
  passwordChangeRequired: "Choose a new password",
  mfaEnrollmentRequired: "Set up your authenticator app",
  mfaRequired: "Enter your verification code",
};

/**
 * Sign-in as the engine directs it: password, then a password change or authenticator
 * setup when required, then the authenticator code (REQ-IDN-004, NFR-19). The engine
 * decides who may continue; this screen only collects the input.
 */
export function SignInScreen({ notice }: { notice?: ReactNode }) {
  const [step, setStep] = useState<SignInStep | null>(null);
  const [keyMode, setKeyMode] = useState(false);
  const [recovering, setRecovering] = useState(false);
  // The engine answers `not_found` for a challenge that expired or was already used.
  const [expired, setExpired] = useState(false);
  const restart = (isExpired: boolean) => {
    setStep(null);
    setExpired(isExpired);
  };
  const advance = (next: SignInStep) => {
    setExpired(false);
    setStep(next);
  };
  return (
    <div className={styles.flow}>
      <h1 className={styles.title}>Sign in</h1>
      {notice}
      <InstallationKeyPanel />
      {recovering ? (
        <RecoverKeyForm
          onClose={() => {
            setRecovering(false);
            setKeyMode(true);
          }}
        />
      ) : step === null ? (
        <>
          {expired && (
            <Alert tone="warning" title="Sign in again">
              This sign-in step expired or was already used. Enter your username and password again.
            </Alert>
          )}
          {keyMode ? <KeySignInForm /> : <CredentialsForm onStep={advance} />}
          <Button onPress={() => setKeyMode(!keyMode)}>
            {keyMode ? "Use password and authenticator app" : "Use a security key"}
          </Button>
          {keyMode && (
            <Button variant="quiet" onPress={() => setRecovering(true)}>
              Recover with a saved code
            </Button>
          )}
        </>
      ) : (
        <>
          <h2 className={styles.stepTitle}>{STEP_TITLES[step.kind]}</h2>
          <StepForm step={step} onStep={setStep} onRestart={restart} />
        </>
      )}
    </div>
  );
}

interface StepFormProps {
  step: SignInStep;
  onStep: (step: SignInStep) => void;
  /** Returns to the username and password step, saying why when the step expired. */
  onRestart: (expired: boolean) => void;
}

function StepForm({ step, onStep, onRestart }: StepFormProps) {
  const onCancel = () => onRestart(false);
  const onExpired = () => onRestart(true);
  switch (step.kind) {
    case "passwordChangeRequired":
      return (
        <InitialPasswordForm
          challengeId={step.challengeId}
          onStep={onStep}
          onCancel={onCancel}
          onExpired={onExpired}
        />
      );
    case "mfaEnrollmentRequired":
      return (
        <MfaEnrollment
          challengeId={step.challengeId}
          enrollment={step.enrollment}
          onCancel={onCancel}
          onExpired={onExpired}
        />
      );
    case "mfaRequired":
      return (
        <MfaCodeForm challengeId={step.challengeId} onCancel={onCancel} onExpired={onExpired} />
      );
  }
}
