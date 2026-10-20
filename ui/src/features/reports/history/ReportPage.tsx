import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import {
  ADD_DELIVERY_OBLIGATION,
  GET_REPORT,
  PREPARE_REPORT_CANDIDATE,
  RECORD_IMPACT_ASSESSMENT,
  type ReportDetail,
  WITHDRAW_REPORT,
} from "../contract";
import { DeliverySection } from "../delivery/DeliverySection";
import { AVAILABILITY } from "../labels";
import { reportPath, workspacePath } from "../paths";
import { ExportsSection } from "./ExportsSection";
import { ImpactSection } from "./ImpactSection";
import { IncidentsSection } from "./IncidentsSection";
import { RevisionsSection } from "./RevisionsSection";
import { WithdrawDialog } from "./WithdrawDialog";

/** Where the breadcrumb leads when the report was not opened from a list. */
const HOME = { label: "Reports", href: "/reports" };

/**
 * One report's history (REQ-RPT-002, REQ-RPT-003, REQ-RPT-013): every revision, impact after
 * release, delivery obligations and attempts, exports, preservation incidents and any
 * withdrawal. Actions are offered only when the report's `allowedActions`, computed by the
 * engine for the signed-in person, names them; the engine still decides. Opening this screen is
 * recorded in the audit trail.
 */
export function ReportPage() {
  const { reportId = "" } = useParams();
  const report = useCommandQuery<ReportDetail>(GET_REPORT, { input: { reportId } });
  const { list, sequence } = useRecordSequence(reportId, HOME, reportPath);
  const data = report.data;
  const availability = data?.availability && AVAILABILITY[data.availability];
  // One header in every state, so Previous and Next keep focus while the next report loads.
  return (
    <Page>
      <PageHeader
        title={data ? `Report ${data.reportNumber}` : "Report"}
        status={
          data && (
            <StatusBadge tone={availability?.tone ?? "neutral"}>
              {availability?.label ?? "Not released"}
            </StatusBadge>
          )
        }
        description="Opening this screen is recorded in the audit trail."
        breadcrumbs={[list, { label: data?.reportNumber ?? "Report", href: reportPath(reportId) }]}
        sequence={sequence}
        actions={data && <ReportActions report={data} />}
      />
      {data && (
        <IdentityBanner
          subject={data.subject}
          specimen={{ accessionNumber: data.accessionNumber }}
        />
      )}
      <RefreshControl query={report} />
      <QueryContent query={report} what="report">
        {(loaded) => <ReportHistory report={loaded} />}
      </QueryContent>
    </Page>
  );
}

/** The current revision, the one released and not yet superseded. */
const currentOf = (report: ReportDetail) =>
  report.revisions.find((each) => each.availability === "current");

/** Preparing the next revision and withdrawing, each when the engine allows it. */
function ReportActions({ report }: { report: ReportDetail }) {
  const navigate = useNavigate();
  const [isWithdrawing, setWithdrawing] = useState(false);
  const allows = (action: string) => report.allowedActions.includes(action);
  const current = currentOf(report);
  return (
    <>
      {allows(PREPARE_REPORT_CANDIDATE) && (
        <Button onPress={() => navigate(workspacePath(report.specimenId))}>
          Prepare next revision
        </Button>
      )}
      {allows(WITHDRAW_REPORT) && current && (
        <Button variant="danger" onPress={() => setWithdrawing(true)}>
          Withdraw report
        </Button>
      )}
      {isWithdrawing && current && (
        <WithdrawDialog
          reportId={report.reportId}
          reportNumber={report.reportNumber}
          currentRevisionNo={current.revisionNo}
          onClose={() => setWithdrawing(false)}
        />
      )}
    </>
  );
}

function ReportHistory({ report }: { report: ReportDetail }) {
  const allows = (action: string) => report.allowedActions.includes(action);
  const current = currentOf(report);
  return (
    <div className={styles.stack}>
      {report.withdrawal && (
        <Alert tone="info" title="This report is withdrawn">
          Withdrawn <Timestamp value={report.withdrawal.withdrawnAt} /> by{" "}
          {report.withdrawal.withdrawnBy.displayName} at revision {report.withdrawal.revisionNo}.
          Reason: {report.withdrawal.reason}
        </Alert>
      )}
      {report.impact && (
        <ImpactSection impact={report.impact} canAssess={allows(RECORD_IMPACT_ASSESSMENT)} />
      )}
      <RevisionsSection
        revisions={report.revisions}
        reportTitle={`Report ${report.reportNumber}`}
      />
      <DeliverySection
        obligations={report.obligations}
        clientId={report.clientId}
        currentRevisionId={allows(ADD_DELIVERY_OBLIGATION) ? current?.revisionId : undefined}
      />
      <ExportsSection exports={report.exports} />
      {<IncidentsSection incidents={report.incidents} revisions={report.revisions} />}
    </div>
  );
}
