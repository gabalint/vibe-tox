import { Link } from "react-router";
import { StatusBadge } from "@/components/StatusBadge";
import type { HostHealthView } from "@/generated/HostHealthView";
import { useCommandQuery } from "@/lib/commands";
import { GET_HOST_HEALTH } from "../contract";
import { healthState } from "../labels";
import styles from "./frame.module.css";

/** How often the frame asks for the health state again; the engine evaluates every 5 minutes. */
export const FRAME_POLL_MS = 60_000;

/**
 * The host's overall health in the top bar, as the engine evaluated it (REQ-OPS-008), linking
 * to the health screen. A failed read shows "unavailable", never a stale state.
 */
export function HealthIndicator() {
  const health = useCommandQuery<HostHealthView>(GET_HOST_HEALTH, undefined, {
    refetchInterval: FRAME_POLL_MS,
  });
  return (
    <Link to="/operations/health" className={styles.healthLink}>
      <HealthBadge health={health} />
    </Link>
  );
}

function HealthBadge({ health }: { health: ReturnType<typeof useCommandQuery<HostHealthView>> }) {
  if (health.isPending) return <StatusBadge tone="neutral">Health: checking</StatusBadge>;
  if (health.isError) return <StatusBadge tone="neutral">Health: unavailable</StatusBadge>;
  const state = healthState(health.data.overall);
  return <StatusBadge tone={state.tone}>{`Health: ${state.label}`}</StatusBadge>;
}
