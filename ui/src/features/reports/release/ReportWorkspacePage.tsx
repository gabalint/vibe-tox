import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import { Dispositions, ReleaseBlocked } from "../content/Dispositions";
import { ReportContent } from "../content/ReportContent";
import {
  GET_REPORT_WORKSPACE,
  PREPARE_REPORT_CANDIDATE,
  type ReportWorkspace,
  type WorkspaceReport,
} from "../contract";
import { AVAILABILITY, COMPLETENESS, RELATIONSHIP, revisionText } from "../labels";
import { candidatePath, reportPath, workspacePath } from "../paths";
import { CandidateSummary } from "./CandidateSummary";
import { PrepareCandidateDialog } from "./PrepareCandidateDialog";

/** Where the breadcrumb leads when the workspace was not opened from a list. */
const HOME = { label: "Release queue", href: "/reports/release" };

/**
 * One specimen's report (REQ-RPT-001, REQ-RPT-005): the would-be content, each expected
 * test's disposition and what blocks release, computed now as a hint; the current revision;
 * and the open candidate. Preparing a candidate freezes the content for review and release.
 * Opening this screen is recorded in the audit trail.
 */
export function ReportWorkspacePage() {
  const { specimenId = "" } = useParams();
  const workspace = useCommandQuery<ReportWorkspace>(GET_REPORT_WORKSPACE, {
    input: { specimenId },
  });
  const { list, sequence } = useRecordSequence(specimenId, HOME, workspacePath);
  const data = workspace.data;
  // One header in every state, so Previous and Next keep focus while the next one loads.
  return (
    <Page>
      <PageHeader
        title={data ? `Report for accession ${data.accessionNumber}` : "Report workspace"}
        breadcrumbs={[
          list,
          { label: data?.accessionNumber ?? "Report workspace", href: workspacePath(specimenId) },
        ]}
        sequence={sequence}
        description="The preview is computed now; release checks everything again. Opening this screen is recorded in the audit trail."
        actions={data && <PrepareAction workspace={data} />}
      />
      {data && (
        <IdentityBanner
          subject={data.subject}
          specimen={{ accessionNumber: data.accessionNumber }}
        />
      )}
      <RefreshControl query={workspace} />
      <QueryContent query={workspace} what="report workspace">
        {(loaded) => <Workspace workspace={loaded} />}
      </QueryContent>
    </Page>
  );
}

/** Preparing a candidate, when the engine allows it; the new candidate's review opens. */
function PrepareAction({ workspace }: { workspace: ReportWorkspace }) {
  const navigate = useNavigate();
  const [isPreparing, setPreparing] = useState(false);
  const { specimenId } = workspace;
  return (
    <>
      {workspace.allowedActions.includes(PREPARE_REPORT_CANDIDATE) && (
        <Button variant="primary" onPress={() => setPreparing(true)}>
          Prepare candidate
        </Button>
      )}
      {isPreparing && (
        <PrepareCandidateDialog
          specimenId={specimenId}
          relationship={workspace.preview.relationship}
          conditionSource={workspace.conditionReviewSource}
          hasIssuedReport={Boolean(workspace.report?.currentRevision)}
          onPrepared={(candidate) => navigate(candidatePath(specimenId, candidate.candidateId))}
          onClose={() => setPreparing(false)}
        />
      )}
    </>
  );
}

function Workspace({ workspace }: { workspace: ReportWorkspace }) {
  const { specimenId, report, preview, openCandidate } = workspace;
  const canDeliver = useHasCapability("deliver_reports");
  const canReviewQuality = useHasCapability("manage_quality");
  return (
    <div className={styles.stack}>
      <ReleaseBlocked gates={preview.gates} />
      {workspace.unavailableCandidateId && (
        <Section title="Open candidate unavailable">
          <p role="alert">
            The earlier candidate failed its integrity check. Its content is not shown and a
            preservation incident has been recorded. Review the current clinical source below and
            prepare a new candidate when its gates pass. The damaged candidate and incident remain
            preserved.
          </p>
          {report && (canDeliver || canReviewQuality) && (
            <Link to={reportPath(report.reportId)}>View report and preservation incidents</Link>
          )}
        </Section>
      )}
      <Facts
        layout="compact"
        items={[
          { label: "Specimen type", value: workspace.specimenTypeCode },
          {
            label: "Report",
            value: report ? (
              <ReportLink report={report} canRead={canDeliver || canReviewQuality} />
            ) : (
              "Not yet issued"
            ),
          },
        ]}
      />
      {openCandidate && (
        <Section title="Open candidate">
          <CandidateSummary candidate={openCandidate} />
          <Link to={candidatePath(specimenId, openCandidate.candidateId)}>Review candidate</Link>
        </Section>
      )}
      <Section title="Release preview">
        {preview.completeness && preview.relationship && (
          <p>
            <StatusBadge tone={COMPLETENESS[preview.completeness].tone}>
              {COMPLETENESS[preview.completeness].label}
            </StatusBadge>{" "}
            {RELATIONSHIP[preview.relationship]}
          </p>
        )}
        <Dispositions dispositions={preview.dispositions} />
      </Section>
      <Section title="Content as it would print">
        {preview.content ? (
          <ReportContent content={preview.content} />
        ) : (
          <p className={styles.hint}>
            Nothing can be shown yet: the engine lists above what is missing.
          </p>
        )}
      </Section>
    </div>
  );
}

function ReportLink({ report, canRead }: { report: WorkspaceReport; canRead: boolean }) {
  const current = report.currentRevision;
  const availability = report.availability && AVAILABILITY[report.availability];
  return (
    <>
      {canRead ? (
        <Link to={reportPath(report.reportId)}>{report.reportNumber}</Link>
      ) : (
        report.reportNumber
      )}{" "}
      {availability && <StatusBadge tone={availability.tone}>{availability.label}</StatusBadge>}
      {current &&
        `, ${revisionText(current.revisionNo, current.relationship)}, ${COMPLETENESS[current.completeness].label.toLowerCase()}`}
    </>
  );
}
