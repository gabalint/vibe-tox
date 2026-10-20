import { type ComponentType, type ReactNode, useContext } from "react";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SetupStatus } from "@/generated/SetupStatus";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { SecurityKeyPrompt } from "@/lib/securityKeyPrompt";
import { AuthLayout } from "./AuthLayout";
import { SETUP_STATUS } from "./contract";
import { DemoSetupContext } from "./demoSetupSlot";
import { IdleLock } from "./IdleLock";
import { LockScreen } from "./LockScreen";
import { SignedOut } from "./SignedOut";

interface AuthGateProps {
  /** Renders the app for the signed-in person. */
  children: (session: SessionInfo) => ReactNode;
  /** Shown beside the signed-out screens, such as listener status (REQ-OPS-013). */
  aside?: ReactNode;
}

/**
 * Shows the app only while the engine reports an unlocked session; otherwise the setup,
 * sign-in or lock screen (REQ-IDN-004). The engine owns the session and decides every action.
 */
export function AuthGate(props: AuthGateProps) {
  return (
    <>
      <AuthGateContent {...props} />
      <SecurityKeyPrompt />
    </>
  );
}

function AuthGateContent({ children, aside }: AuthGateProps) {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const DemoSetup = useContext(DemoSetupContext);
  const setup = useCommandQuery<SetupStatus>(SETUP_STATUS, undefined, {
    enabled: DemoSetup !== null && session.isSuccess && !!session.data && !session.data.locked,
  });
  // A failed refresh keeps the old data; the engine's latest answer wins (idle lock).
  if (session.isSuccess && session.data && !session.data.locked) {
    const progress = DemoSetup ? demoSetupView(setup, DemoSetup) : null;
    return (
      <>
        <IdleLock idleTimeoutSeconds={session.data.idleTimeoutSeconds} />
        {progress ? <AuthLayout aside={aside}>{progress}</AuthLayout> : children(session.data)}
      </>
    );
  }
  return <AuthLayout aside={aside}>{signedOutView(session)}</AuthLayout>;
}

function demoSetupView(
  setup: ReturnType<typeof useCommandQuery<SetupStatus>>,
  DemoSetup: ComponentType,
) {
  if (setup.isPending) return <LoadingState label="Checking setup…" />;
  if (setup.isError) {
    return (
      <ErrorState
        title="The setup status could not be checked"
        error={setup.error}
        onRetry={() => void setup.refetch()}
      />
    );
  }
  return setup.data.demoSetupPending ? <DemoSetup /> : null;
}

// Without a session the sign-in form is always offered: it grants nothing by itself, and
// only a session reported by the engine opens the app.
function signedOutView(session: ReturnType<typeof useCommandQuery<SessionInfo | null>>) {
  if (session.isPending) return <LoadingState label="Checking sign-in…" />;
  if (session.isError) {
    return (
      <SignedOut
        notice={
          <ErrorState
            title="The sign-in status could not be checked"
            error={session.error}
            onRetry={() => void session.refetch()}
          />
        }
      />
    );
  }
  if (session.data?.locked) return <LockScreen session={session.data} />;
  return <SignedOut />;
}
