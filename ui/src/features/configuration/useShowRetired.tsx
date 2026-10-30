import { Checkbox } from "@/components/Checkbox";
import { useSearchParam } from "@/lib/searchParam";
import type { DirectoryFilters } from "./Directory";

/** The address parameter of "Show retired", `?retired=show`, for "Clear filters". */
export const RETIRED_PARAM = "retired";
const SHOW = "show";

/**
 * "Show retired" for a directory that hides retired records: the same wording, address
 * parameter and count label on every such list. `many` names the records, such as
 * "destinations". Retired records are out of the count until shown.
 */
export function useShowRetired(
  many: string,
): Pick<DirectoryFilters<object>, "moreFilters" | "scopeLabel"> & { showRetired: boolean } {
  const [retired, setRetired] = useSearchParam(RETIRED_PARAM);
  const showRetired = retired === SHOW;
  return {
    showRetired,
    moreFilters: {
      content: (
        <Checkbox isSelected={showRetired} onChange={(show) => setRetired(show ? SHOW : null)}>
          Show retired
        </Checkbox>
      ),
      applied: showRetired ? `Retired ${many} included` : undefined,
    },
    scopeLabel: showRetired ? undefined : "retired excluded",
  };
}
