import { useCallback } from "react";
import type { AnalyteView } from "@/generated/AnalyteView";
import { useCommandQuery } from "@/lib/commands";
import { LIST_ANALYTES } from "./contract";

/**
 * A catalog analyte's name and code, such as "Synthetic amphetamine (SYN-AMP)", for showing
 * a source's test-code map. Stable while the catalog is unchanged, so grid columns built from
 * it are too.
 */
export function useAnalyteName(): (id: string) => string {
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const catalog = analytes.data;
  return useCallback(
    (id: string) => {
      const analyte = catalog?.find((candidate) => candidate.id === id);
      return analyte ? `${analyte.name} (${analyte.code})` : id;
    },
    [catalog],
  );
}
