import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { QueryContent } from "@/components/QueryContent";
import type { TargetView } from "@/generated/TargetView";
import { allowedRowActions } from "@/lib/allowedActions";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { useSearchParam } from "@/lib/searchParam";
import { LIST_BACKUP_TARGETS } from "../contract";
import { type TargetDialogState, TargetDialogs } from "./TargetDialogs";
import { TargetPanel } from "./TargetPanel";
import { TargetsToolbar } from "./TargetsToolbar";
import { TARGET_ACTIONS } from "./targetActions";
import { TARGET_COLUMNS } from "./targetColumns";

const getRowId = (row: TargetView) => row.targetId;

/**
 * The folders backups and audit anchors go to (design brief §3.3, D3): a second internal disk,
 * a removable drive in rotation, or a network share. Each folder is chosen in a dialog the app
 * opens, never typed. A target opens in a side panel with its sets and the actions the engine
 * allows; retiring a target keeps the copies already on it.
 */
export function TargetsSection() {
  const targets = useCommandQuery<TargetView[]>(LIST_BACKUP_TARGETS);
  const record = useOpenRecord();
  const [retired, setRetired] = useSearchParam("retired");
  const showRetired = retired === "show";
  const setShowRetired = (show: boolean) => setRetired(show ? "show" : null);
  const [dialog, setDialog] = useState<TargetDialogState>(null);
  const all = targets.data;
  const rows = useMemo(
    () => (showRetired ? (all ?? []) : (all ?? []).filter((target) => !target.retired)),
    [all, showRetired],
  );
  const rowActions = allowedRowActions(TARGET_ACTIONS, {
    label: (row: TargetView) => row.label,
    allowed: (row) => row.allowedActions,
    onAction: (kind, target) => setDialog({ kind, target }),
  });
  const add = <Button onPress={() => setDialog({ kind: "add" })}>Add target</Button>;
  const panel = openRecordPanel(
    record,
    all,
    getRowId,
    (target) => (
      <TargetPanel
        target={target}
        onAction={(kind) => setDialog({ kind, target })}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
    <p>This backup target does not exist.</p>,
  );
  return (
    <>
      <DetailLayout panel={panel}>
        <TargetsToolbar
          add={add}
          count={targets.isSuccess ? `${rows.length} of ${all?.length ?? 0} targets` : undefined}
          showRetired={showRetired}
          onShowRetired={setShowRetired}
        />
        <QueryContent
          query={targets}
          what="backup targets"
          loadingRows={4}
          empty={{
            when: (data) => data.length === 0,
            title: "No backup targets",
            description:
              "Nothing is backed up until a target is added. Add a network share or a second disk, and a removable drive to rotate off site.",
            action: add,
          }}
        >
          {() =>
            rows.length === 0 ? (
              <EmptyState
                title="No active backup targets"
                description="Every target is retired."
                action={<Button onPress={() => setShowRetired(true)}>Show retired targets</Button>}
              />
            ) : (
              <DataGrid
                label="Backup targets"
                columns={TARGET_COLUMNS}
                rows={rows}
                getRowId={getRowId}
                openRowId={record.openId}
                onOrderChange={record.followOrder}
                onRowAction={(row, order) => record.open(row.targetId, order)}
                rowActions={rowActions}
                fill
              />
            )
          }
        </QueryContent>
      </DetailLayout>
      <TargetDialogs dialog={dialog} onClose={() => setDialog(null)} />
    </>
  );
}
