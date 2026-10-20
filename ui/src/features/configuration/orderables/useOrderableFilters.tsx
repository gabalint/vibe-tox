import { useMemo } from "react";
import { Select } from "@/components/Select";
import type { OrderableView } from "@/generated/OrderableView";
import { useSearchParam } from "@/lib/searchParam";
import { type DirectoryFilters, useClearParams } from "../Directory";

const ALL = "all";
const PARAMS = ["kind", "state"];

const KINDS = [
  { id: ALL, label: "All kinds" },
  { id: "test", label: "Tests" },
  { id: "panel", label: "Panels" },
];

/**
 * Version states, each read from the engine's version numbers on the row; nothing here
 * decides which version is in effect.
 */
const STATES: readonly { id: string; label: string; keep: (row: OrderableView) => boolean }[] = [
  { id: "awaiting-approval", label: "Awaiting approval", keep: (row) => row.draftVersion !== null },
  { id: "in-effect", label: "In effect", keep: (row) => row.versionInEffect !== null },
  {
    id: "none-in-effect",
    label: "No version in effect",
    keep: (row) => row.versionInEffect === null,
  },
];

const STATE_OPTIONS = [{ id: ALL, label: "Any state" }, ...STATES];

/** A filter's value from the URL; an unknown value filters nothing. */
function filterValue(param: string | null, options: readonly { id: string }[]): string {
  return options.find((option) => option.id === param)?.id ?? ALL;
}

/**
 * The kind and version-state filters of tests and panels, in the URL as `?kind=` and
 * `?state=`. Changing one replaces the history entry and keeps the find text.
 */
export function useOrderableFilters(): DirectoryFilters<OrderableView> {
  const [kindParam, setKind] = useSearchParam("kind");
  const [stateParam, setState] = useSearchParam("state");
  const clear = useClearParams(PARAMS);
  const kind = filterValue(kindParam, KINDS);
  const versionState = filterValue(stateParam, STATES);
  const keep = useMemo(() => {
    const stateKeep = STATES.find((each) => each.id === versionState)?.keep;
    return (row: OrderableView) =>
      (kind === ALL || row.kind === kind) && (stateKeep === undefined || stateKeep(row));
  }, [kind, versionState]);
  return {
    controls: (
      <>
        <Select
          layout="inline"
          label="Kind"
          name="kind"
          options={KINDS}
          value={kind}
          onChange={(value) => setKind(value === ALL ? null : value)}
        />
        <Select
          layout="inline"
          label="State"
          name="state"
          options={STATE_OPTIONS}
          value={versionState}
          onChange={(value) => setState(value === ALL ? null : value)}
        />
      </>
    ),
    isSet: kind !== ALL || versionState !== ALL,
    keep,
    clear,
  };
}
