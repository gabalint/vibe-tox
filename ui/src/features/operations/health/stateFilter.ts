import type { HealthCheckView } from "@/generated/HealthCheckView";
import { type Choice, choiceOf } from "@/lib/choices";
import { useSearchParam } from "@/lib/searchParam";
import { healthState } from "../labels";

/** The groups the checks are counted and filtered by. */
export type StateFilter = "alert" | "warning" | "ok";

/** Each group, worded as its badge, most urgent first. */
export const STATE_FILTERS: readonly Choice<StateFilter>[] = (
  ["alert", "warning", "ok"] as const
).map((id) => ({ id, label: healthState(id).label }));

/**
 * The group of each state, as the engine counts it in the overall state: Unknown with
 * Warning, so it is never counted as fine, and Attested with OK. A state not listed is in no
 * group and shows only under All. Presentation only: the engine decides every state.
 */
const GROUP: Readonly<Record<string, StateFilter>> = {
  alert: "alert",
  warning: "warning",
  unknown: "warning",
  ok: "ok",
  attested: "ok",
};

/** How many checks each group holds. */
export function countByState(checks: readonly HealthCheckView[]): Record<StateFilter, number> {
  const counts = { alert: 0, warning: 0, ok: 0 };
  for (const check of checks) {
    const group = GROUP[check.state];
    if (group) counts[group] += 1;
  }
  return counts;
}

/** The checks of `filter`'s group; every check when there is no filter. */
export function checksIn(
  checks: readonly HealthCheckView[],
  filter: StateFilter | null,
): readonly HealthCheckView[] {
  return filter === null ? checks : checks.filter((check) => GROUP[check.state] === filter);
}

/**
 * The state filter in the address (`?state=alert`), so a reload or Back keeps it; an unknown
 * value shows every check.
 */
export function useStateFilter(): [StateFilter | null, (next: StateFilter | null) => void] {
  const [value, setValue] = useSearchParam("state");
  return [choiceOf(STATE_FILTERS, value), setValue];
}
