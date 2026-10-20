import { StatusBadge } from "@/components/StatusBadge";
import { healthState, type Status } from "../labels";

// Labels for the health and run screens. Unknown codes show as received; nothing here decides a
// state, a threshold or a due time.

/** Shown for a value the engine did not send. */
export const NONE = "—";

/** Keyed by `KeepAwakeState`. */
const KEEP_AWAKE: Record<string, Status> = {
  held: { label: "Keeping the computer awake", tone: "neutral" },
  not_needed: { label: "Not needed: no analyzer listener is active", tone: "neutral" },
  unavailable: { label: "Unavailable", tone: "warning" },
};

export function keepAwakeState(state: string): Status {
  return KEEP_AWAKE[state] ?? { label: state, tone: "neutral" };
}

const STOP_KINDS: Record<string, string> = {
  clean: "Closed normally",
  quit_with_warnings: "Quit despite warnings",
};

export function stopKindText(kind: string | null): string {
  if (kind === null) return NONE;
  return STOP_KINDS[kind] ?? kind;
}

/**
 * How urgently a state needs attention, most urgent first, to list the checks needing action
 * at the top (UI review finding 21). `unknown` ranks with `warning`, as the engine counts it;
 * a state not listed ranks last. Presentation order only: the engine decides every state.
 */
const SEVERITY: Readonly<Record<string, number>> = {
  alert: 0,
  warning: 1,
  unknown: 1,
  attested: 2,
  ok: 3,
};

export function severityRank(state: string): number {
  return SEVERITY[state] ?? 4;
}

/** A health state as a badge with its text. */
export function HealthBadge({ state }: { state: string }) {
  const status = healthState(state);
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}
