import type { DataGridColumn } from "@/components/DataGrid";
import type { LotView } from "@/generated/LotView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_LOTS } from "../analytics/contract";
import { expiresIn, MATERIAL_KINDS } from "../analytics/labels";
import type { WorkQueueItem } from "./contract";

/** The queue whose items are lots, shown with the lot's own columns (#4557). */
export const LOTS_EXPIRING_SOON = "lots_expiring_soon";

/**
 * The columns of Lots expiring soon: each item is a lot, named by its ID, whose details come
 * from the lot list. Until that list loads, an item shows its lot number and next action only.
 */
export function useLotColumns(enabled: boolean): readonly DataGridColumn<WorkQueueItem>[] {
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: true } }, { enabled });
  const byId = new Map((lots.data ?? []).map((lot) => [lot.lotId, lot]));
  const lot = (row: WorkQueueItem) => byId.get(row.itemId);
  const text = (value: (lot: LotView) => string | null) => (row: WorkQueueItem) => {
    const found = lot(row);
    return (found && value(found)) ?? "—";
  };
  const daysLeft = (row: WorkQueueItem) => lot(row)?.daysLeft ?? null;
  return [
    {
      id: "lot",
      header: "Lot",
      cell: (row) => row.title,
      sortValue: (row) => row.title,
      identifier: true,
      width: 140,
    },
    { id: "product", header: "Product", cell: text((found) => found.productName) },
    {
      id: "material",
      header: "Material",
      cell: text((found) => labelOf(MATERIAL_KINDS, found.materialKind)),
      width: 150,
    },
    { id: "sample", header: "Sample label", cell: text((found) => found.sampleLabel), width: 140 },
    {
      id: "expires",
      header: "Expires",
      cell: text((found) => found.expiresOn),
      sortValue: text((found) => found.expiresOn),
      width: 120,
    },
    {
      id: "daysLeft",
      header: "Days left",
      cell: (row) => {
        const days = daysLeft(row);
        return days === null ? (row.stateLabel ?? "—") : expiresIn(days);
      },
      sortValue: (row) => daysLeft(row) ?? Number.MAX_SAFE_INTEGER,
      width: 160,
    },
    { id: "next", header: "Next action", cell: (row) => row.nextAction },
  ];
}
