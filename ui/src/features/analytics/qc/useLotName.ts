import { useCallback } from "react";
import type { LotView } from "@/generated/LotView";
import { useCommandQuery } from "@/lib/commands";
import { LIST_LOTS } from "../contract";
import { lotText } from "../labels";

/** A lot's product and lot number, for the QC observations and deviations of a run. */
export function useLotName(): (lotId: string | null) => string {
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: false } });
  return useCallback(
    (id: string | null) => {
      if (id === null) return "—";
      const lot = lots.data?.find((candidate) => candidate.lotId === id);
      return lot ? lotText(lot) : id;
    },
    [lots.data],
  );
}
