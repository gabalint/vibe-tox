import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { FindCount } from "@/components/FindField";
import { QueryContent } from "@/components/QueryContent";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { Toolbar } from "@/components/Toolbar";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import type { WithdrawUpdateApprovalInput } from "@/generated/WithdrawUpdateApprovalInput";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { LIST_UPDATE_APPROVALS, VERSION_QUERIES, WITHDRAW_UPDATE_APPROVAL } from "../contract";
import { ApprovalPanel } from "./ApprovalPanel";
import { APPROVAL_COLUMNS, isPending } from "./approvalColumns";
import { RecordApprovalDialog } from "./RecordApprovalDialog";

const approvalId = (row: UpdateApprovalView) => row.approvalId;
const ACTIONS: readonly OfferedAction<typeof WITHDRAW_UPDATE_APPROVAL>[] = [
  { id: WITHDRAW_UPDATE_APPROVAL, label: "Withdraw approval", variant: "danger" },
];
/** Interim: `isPending` stands in for the engine's `allowedActions`. */
const allowed = (approval: UpdateApprovalView) =>
  isPending(approval) ? [WITHDRAW_UPDATE_APPROVAL] : [];

/**
 * Every update approval, newest first (REQ-OPS-006, design brief §7), for people with
 * `manage_configuration`. An approval opens in a side panel and can be withdrawn there, or
 * from the row's menu, until a run of its version starts.
 */
export function ApprovalsSection() {
  const query = useCommandQuery<UpdateApprovalView[]>(LIST_UPDATE_APPROVALS);
  const record = useOpenRecord();
  const [withdrawing, setWithdrawing] = useState<UpdateApprovalView | null>(null);
  const [recording, setRecording] = useState(false);
  const actionsOf = (approval: UpdateApprovalView) =>
    allowedActionItems(ACTIONS, allowed(approval), () => setWithdrawing(approval));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: UpdateApprovalView) => `approval of version ${row.version}`,
    allowed,
    onAction: (_, row) => setWithdrawing(row),
  });
  const panel = openRecordPanel(
    record,
    query.data,
    approvalId,
    (approval) => (
      <ApprovalPanel
        approval={approval}
        actions={actionsOf(approval)}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
    <p>This update approval does not exist.</p>,
  );
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          actions={
            <Button onPress={() => setRecording(true)}>
              Approve a package from outside the app
            </Button>
          }
          count={
            query.isSuccess && (
              <FindCount
                shown={query.data.length}
                total={query.data.length}
                noun="update approvals"
                one="update approval"
              />
            )
          }
        />
        <QueryContent
          query={query}
          what="update approvals"
          loadingRows={4}
          empty={{
            when: (rows) => rows.length === 0,
            title: "No update approvals",
            description: "Record an approval before installing an update.",
          }}
        >
          {(rows) => (
            <DataGrid
              label="Update approvals"
              columns={APPROVAL_COLUMNS}
              rows={rows}
              getRowId={approvalId}
              openRowId={record.openId}
              onOrderChange={record.followOrder}
              onRowAction={(row, order) => record.open(row.approvalId, order)}
              rowActions={rowActions}
              fill
            />
          )}
        </QueryContent>
      </DetailLayout>
      {recording && <RecordApprovalDialog onClose={() => setRecording(false)} />}
      {withdrawing && (
        <WithdrawDialog approval={withdrawing} onClose={() => setWithdrawing(null)} />
      )}
    </>
  );
}

function WithdrawDialog({
  approval,
  onClose,
}: {
  approval: UpdateApprovalView;
  onClose: () => void;
}) {
  const withdraw = useCommandMutation<UpdateApprovalView, { input: WithdrawUpdateApprovalInput }>(
    WITHDRAW_UPDATE_APPROVAL,
    { invalidates: VERSION_QUERIES },
  );
  return (
    <ReasonConfirmDialog
      title={`Withdraw the approval of version ${approval.version}?`}
      confirmLabel="Withdraw approval"
      variant="danger"
      command={withdraw}
      input={(reason) => ({
        approvalId: approval.approvalId,
        expectedRevision: approval.revision,
        reason,
      })}
      successTitle={`Approval of version ${approval.version} withdrawn`}
      onClose={onClose}
    >
      <p>
        The approval stays on record as withdrawn. If this version is installed without a new
        approval, the app raises an alert that it started without one.
      </p>
    </ReasonConfirmDialog>
  );
}
