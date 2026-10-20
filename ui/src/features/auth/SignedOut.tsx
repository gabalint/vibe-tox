import { type ReactNode, useState } from "react";
import { Alert } from "@/components/Alert";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { SetupStatus } from "@/generated/SetupStatus";
import { useCommandQuery } from "@/lib/commands";
import { SETUP_STATUS } from "./contract";
import { SetupScreen } from "./SetupScreen";
import { SignInScreen } from "./SignInScreen";

/** First-run setup while the engine has no users; the sign-in screen otherwise. */
export function SignedOut({ notice }: { notice?: ReactNode }) {
  const setup = useCommandQuery<SetupStatus>(SETUP_STATUS);
  const [created, setCreated] = useState(false);
  if (setup.isPending) return <LoadingState label="Checking setup…" />;
  if (setup.isSuccess && setup.data.needsAdministrator) {
    return <SetupScreen onCreated={() => setCreated(true)} />;
  }
  return <SignInScreen notice={signInNotice(notice, setup, created)} />;
}

function signInNotice(
  notice: ReactNode,
  setup: ReturnType<typeof useCommandQuery<SetupStatus>>,
  created: boolean,
): ReactNode {
  if (notice) return notice;
  if (setup.isError) {
    return (
      <ErrorState
        title="The setup status could not be checked"
        error={setup.error}
        onRetry={() => void setup.refetch()}
      />
    );
  }
  if (created) {
    return (
      <Alert tone="success">
        The administrator account was created. Sign in with it to set up the authenticator app.
      </Alert>
    );
  }
  return null;
}
