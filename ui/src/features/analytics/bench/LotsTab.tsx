import { useMemo, useState } from "react";
import { DataGrid, type DataGridColumn, type GridRowActions } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { NoResults } from "@/components/FindField";
import { ListCount } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { Select } from "@/components/Select";
import { StatusBadge } from "@/components/StatusBadge";
import { Toolbar } from "@/components/Toolbar";
import type { LotView } from "@/generated/LotView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { type Choice, choiceOf, labelOf } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { type OpenRecord, useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { useSearchParam } from "@/lib/searchParam";
import { LIST_LOTS, MANAGE_QUALITY } from "../contract";
import { lotStatus, MATERIAL_KINDS } from "../labels";
import { LotPanel } from "./LotPanel";
import { RegisterLotDialog } from "./RegisterLotDialog";
import { WithdrawLotDialog } from "./WithdrawLotDialog";

const COLUMNS: readonly DataGridColumn<LotView>[] = [
  {
    id: "lot",
    header: "Lot",
    identifier: true,
    cell: (row) => row.lotNumber,
    sortValue: (row) => row.lotNumber,
    width: 140,
  },
  { id: "product", header: "Product", cell: (row) => row.productName },
  {
    id: "kind",
    header: "Material",
    cell: (row) => labelOf(MATERIAL_KINDS, row.materialKind),
    sortValue: (row) => row.materialKind,
    width: 150,
  },
  { id: "level", header: "Level", cell: (row) => row.levelText ?? "—", width: 120 },
  { id: "sample", header: "Sample label", cell: (row) => row.sampleLabel ?? "—", width: 140 },
  {
    id: "expires",
    header: "Expires",
    cell: (row) => row.expiresOn,
    sortValue: (row) => row.expiresOn,
    width: 120,
  },
  {
    id: "status",
    header: "Status",
    cell: (row) => {
      const status = lotStatus(row);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    width: 140,
  },
];

const getRowId = (lot: LotView) => lot.lotId;

type Shown = "active" | "all";

const SHOWN: readonly Choice<Shown>[] = [
  { id: "active", label: "Active lots" },
  { id: "all", label: "All lots, with withdrawn and expired" },
];

/** Withdrawn and expired lots are hidden unless `?show=all`; the engine reports both states. */
function useShownParam(): [Shown, (shown: Shown) => void] {
  const [shown, setShown] = useSearchParam("show");
  return [choiceOf(SHOWN, shown) ?? "active", (next) => setShown(next === "all" ? next : null)];
}

const ACTIONS: readonly OfferedAction<"withdraw_lot">[] = [
  { id: "withdraw_lot", label: "Withdraw lot", variant: "danger" },
];

/**
 * Reagent, calibrator, control and internal-standard lots (REQ-ANA-007), opening in a panel.
 * Expiry is derived by the engine; an expired or withdrawn lot cannot be used on a new run.
 * Lots carry no `allowedActions` yet: quality staff may withdraw an active lot.
 */
export function LotsTab({
  isRegistering,
  onCloseRegistration,
}: {
  isRegistering: boolean;
  onCloseRegistration: () => void;
}) {
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: false } });
  const canManage = useHasCapability(MANAGE_QUALITY);
  const record = useOpenRecord();
  const [shown, setShown] = useShownParam();
  const [withdrawn, setWithdrawn] = useState<LotView | null>(null);
  const allowed = (lot: LotView) => (canManage && lot.state === "active" ? ["withdraw_lot"] : []);
  const withdraw = (lot: LotView) => setWithdrawn(lot);
  const rows = useMemo(
    () =>
      lots.data &&
      (shown === "all"
        ? lots.data
        : lots.data.filter((lot) => lot.state === "active" && !lot.expired)),
    [lots.data, shown],
  );
  const panel = openRecordPanel(record, lots.data, getRowId, (open) => (
    <LotPanel
      lot={open}
      actions={allowedActionItems(ACTIONS, allowed(open), () => withdraw(open))}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (lot: LotView) => lot.lotNumber,
    allowed,
    onAction: (_, lot) => withdraw(lot),
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={
            rows && (
              <ListCount
                shown={rows.length}
                noun={shown === "all" ? "lots" : "active lots"}
                one={shown === "all" ? "lot" : "active lot"}
              />
            )
          }
        >
          <Select
            layout="inline"
            label="Show"
            name="show"
            options={SHOWN}
            value={shown}
            onChange={(next) => setShown(choiceOf(SHOWN, next) ?? "active")}
          />
        </Toolbar>
        <QueryContent
          query={lots}
          what="lots"
          loadingRows={6}
          empty={{
            when: (rows) => rows.length === 0,
            title: "No lots yet",
            description: "Register the lots in use before creating runs.",
          }}
        >
          {() => (
            <LotList
              rows={rows ?? []}
              onShowAll={() => setShown("all")}
              record={record}
              rowActions={rowActions}
            />
          )}
        </QueryContent>
      </DetailLayout>
      {isRegistering && (
        <RegisterLotDialog onDone={(lot) => record.open(lot.lotId)} onClose={onCloseRegistration} />
      )}
      {withdrawn && <WithdrawLotDialog lot={withdrawn} onClose={() => setWithdrawn(null)} />}
    </>
  );
}

interface LotListProps {
  rows: readonly LotView[];
  onShowAll: () => void;
  record: OpenRecord;
  rowActions: GridRowActions<LotView>;
}

function LotList({ rows, onShowAll, record, rowActions }: LotListProps) {
  if (rows.length === 0) return <NoResults noun="lots" query="" onClear={onShowAll} />;
  return (
    <DataGrid
      label="Lots"
      columns={COLUMNS}
      rows={rows}
      getRowId={getRowId}
      onRowAction={(lot, order) => record.open(lot.lotId, order)}
      openRowId={record.openId}
      onOrderChange={record.followOrder}
      rowActions={rowActions}
      fill
    />
  );
}
