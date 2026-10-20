import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import type { HostHealthView } from "@/generated/HostHealthView";
import { useCommandQuery } from "@/lib/commands";
import {
  GET_HOST_HEALTH,
  GET_RECOVERY_RECONCILIATION,
  type RecoveryReconciliation,
} from "../contract";
import { FRAME_POLL_MS } from "./HealthIndicator";
import { VersionNotice } from "./VersionNotice";

/** The gate message of an open recovery reconciliation (design brief §3.8). */
const RECOVERY_GATE =
  "Restored from a backup: release, delivery and billing handoff wait until the laboratory closes the recovery reconciliation.";

/**
 * Banners every signed-in person sees: an open recovery reconciliation (REQ-OPS-002, D5) and a
 * running version the laboratory has not approved or accepted (REQ-OPS-006, L9). Both come from
 * the engine; a failed read shows no banner, and the health indicator says the state is
 * unavailable. Version notices can be hidden until their identity changes or the signed-in
 * frame remounts. Recovery notices stay visible while their release gate applies.
 */
export function OperationsBanners() {
  const reconciliation = useCommandQuery<RecoveryReconciliation | null>(
    GET_RECOVERY_RECONCILIATION,
    undefined,
    { refetchInterval: FRAME_POLL_MS },
  );
  const health = useCommandQuery<HostHealthView>(GET_HOST_HEALTH, undefined, {
    refetchInterval: FRAME_POLL_MS,
  });
  const recoveryOpen = reconciliation.data != null && reconciliation.data.closed === null;
  const version = health.data?.checks.find(
    (check) => check.check === "version" && (check.state === "warning" || check.state === "alert"),
  );
  return (
    <>
      {recoveryOpen && (
        <Alert tone="warning" title="Recovery reconciliation open">
          {RECOVERY_GATE} <Link to="/operations/recovery">Open the recovery reconciliation</Link>
        </Alert>
      )}
      {version && (
        <VersionNotice
          key={JSON.stringify([
            health.data?.host.appVersion,
            version.state,
            version.detailCode,
            version.summary,
          ])}
          version={version}
        />
      )}
    </>
  );
}
