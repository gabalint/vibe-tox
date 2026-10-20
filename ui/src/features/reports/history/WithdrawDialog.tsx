import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import {
  REPORT_QUERIES,
  type ReportDetail,
  WITHDRAW_REPORT,
  type WithdrawReportInput,
} from "../contract";

interface WithdrawDialogProps {
  reportId: string;
  reportNumber: string;
  currentRevisionNo: number;
  onClose: () => void;
}

/**
 * Withdraws the whole report (REQ-RPT-002, REQ-RPT-013). Withdrawal is terminal and is never
 * blocked by holds: it is the safety action. A notice without results, demographics or the
 * reason goes to every destination that had an obligation on the report.
 */
export function WithdrawDialog({
  reportId,
  reportNumber,
  currentRevisionNo,
  onClose,
}: WithdrawDialogProps) {
  return (
    <ReauthConfirmDialog<Record<string, never>, WithdrawReportInput, ReportDetail>
      title={`Withdraw report ${reportNumber}`}
      confirmLabel="Withdraw report"
      variant="danger"
      command={{ name: WITHDRAW_REPORT, invalidates: REPORT_QUERIES }}
      extraDefaults={{}}
      withReason
      toInput={({ reason, password }) => ({
        reportId,
        expectedRevisionNo: currentRevisionNo,
        reason: reason.trim(),
        password,
      })}
      successTitle={() => `Report ${reportNumber} withdrawn`}
      onClose={onClose}
      errorTitle="Not withdrawn"
    >
      <p>
        Every revision of report {reportNumber} becomes withdrawn, and no further revision can be
        issued. This cannot be undone. A withdrawal notice, without results or the reason, is queued
        for every destination that was sent the report.
      </p>
    </ReauthConfirmDialog>
  );
}
