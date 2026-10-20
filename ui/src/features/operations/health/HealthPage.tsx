import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { useToast } from "@/components/Toast";
import type { CommandError } from "@/generated/CommandError";
import type { HostHealthView } from "@/generated/HostHealthView";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { toUserMessage } from "@/lib/errors";
import {
  GET_HOST_HEALTH,
  HEALTH_QUERIES,
  REFRESH_HOST_HEALTH,
  REFRESH_RATE_LIMITED,
} from "../contract";
import { ChecksSection } from "./ChecksSection";
import { HostSection } from "./HostSection";

/**
 * Host health (REQ-OPS-008, REQ-OPS-013), for any session: each check as the engine evaluated
 * it, this computer's facts and keep-awake. "Check now" asks the engine to probe again; it
 * allows that once a minute and says so when asked sooner.
 */
export function HealthPage() {
  const toast = useToast();
  const health = useCommandQuery<HostHealthView>(GET_HOST_HEALTH);
  const refresh = useCommandMutation<HostHealthView>(REFRESH_HOST_HEALTH, {
    invalidates: HEALTH_QUERIES,
  });
  const checkNow = () =>
    refresh.mutate(
      {},
      { onSuccess: () => toast.show({ tone: "info", title: "Checks run again" }) },
    );
  return (
    <Page>
      <PageHeader
        title="Host health"
        description="Backups, audit anchors, disk, operating system and listeners, as the app last checked them."
        actions={
          <>
            <Link to="/operations/health/history">Health history</Link>
            <Button isPending={refresh.isPending} onPress={checkNow}>
              Check now
            </Button>
          </>
        }
      />
      {refresh.error && <RefreshRefusal error={refresh.error} />}
      <QueryContent query={health} what="host health" loadingRows={8}>
        {(data) => (
          <>
            <HostSection health={data} />
            <ChecksSection health={data} />
          </>
        )}
      </QueryContent>
    </Page>
  );
}

/** A refresh within a minute of the previous one is expected, not a failure. */
function RefreshRefusal({ error }: { error: CommandError }) {
  if (error.gateReasons.some((gate) => gate.code === REFRESH_RATE_LIMITED)) {
    return (
      <Alert tone="info" title="Checked less than a minute ago">
        {toUserMessage(error)}
      </Alert>
    );
  }
  return <ErrorState title="Not checked again" error={error} />;
}
