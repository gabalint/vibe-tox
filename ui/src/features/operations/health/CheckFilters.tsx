import { Button } from "@/components/Button";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import type { HealthCheckView } from "@/generated/HealthCheckView";
import { choiceOf } from "@/lib/choices";
import { countByState, STATE_FILTERS, type StateFilter } from "./stateFilter";

/** The option for "no filter". */
const ALL = "all";

interface CheckFiltersProps {
  checks: readonly HealthCheckView[];
  filter: StateFilter | null;
  onApply: (filter: StateFilter | null) => void;
}

/**
 * The State filter and the count of checks in each state ("6 Alert, 2 Warning, 5 OK"). Pressing
 * a count applies the same filter as choosing it; All shows every check again.
 */
export function CheckFilters({ checks, filter, onApply }: CheckFiltersProps) {
  const counts = countByState(checks);
  return (
    <Toolbar
      variant="collection"
      align="end"
      actions={STATE_FILTERS.map(({ id, label }) => (
        <Button key={id} variant="quiet" isDisabled={counts[id] === 0} onPress={() => onApply(id)}>
          {`${counts[id]} ${label}`}
        </Button>
      ))}
    >
      <Select
        label="State"
        name="state"
        options={[{ id: ALL, label: "All" }, ...STATE_FILTERS]}
        value={filter ?? ALL}
        onChange={(next) => onApply(choiceOf(STATE_FILTERS, next))}
      />
    </Toolbar>
  );
}
