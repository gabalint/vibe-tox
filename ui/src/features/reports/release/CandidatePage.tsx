import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { GateReasons } from "@/components/ErrorState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { useCommandQuery } from "@/lib/commands";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import { Dispositions, ReleaseBlocked } from "../content/Dispositions";
import { ReportContent } from "../content/ReportContent";
import {
  type CandidateView,
  GET_REPORT_CANDIDATE,
  GET_REPORT_WORKSPACE,
  RELEASE_REPORT,
  type ReportWorkspace,
} from "../contract";
import { ParentIdentity } from "../ParentIdentity";
import { candidatePath, workspacePath } from "../paths";
import { CandidateSummary } from "./CandidateSummary";
import { ReleaseDialog } from "./ReleaseDialog";

/**
 * Candidate review (REQ-RPT-001): exactly the printed content (`ToxReportData`), each test's
 * disposition, the gates and whether the content changed since preparation, all recomputed by
 * the engine on read. Release is offered only when the engine allows it. The patient comes
 * from the specimen's report workspace, the page the candidate is opened from, since the
 * candidate carries no identity. Opening this screen is recorded in the audit trail.
 */
export function CandidatePage() {
  const { specimenId = "", candidateId = "" } = useParams();
  const candidate = useCommandQuery<CandidateView>(GET_REPORT_CANDIDATE, {
    input: { candidateId },
  });
  // Engine request: `subject` and `accessionNumber` on `CandidateView`, so this second
  // audited read can go.
  const workspace = useCommandQuery<ReportWorkspace>(GET_REPORT_WORKSPACE, {
    input: { specimenId },
  });
  const pathOf = (id: string) => candidatePath(specimenId, id);
  // A candidate is reached from its specimen's report workspace.
  const home = { label: "Report workspace", href: workspacePath(specimenId) };
  const { list, sequence } = useRecordSequence(candidateId, home, pathOf);
  const data = candidate.data;
  // One header in every state, so Previous and Next keep focus while the next one loads.
  return (
    <Page>
      <PageHeader
        title={data ? `Candidate for report ${data.reportNumber}` : "Report candidate"}
        description="Review exactly what will print. Opening this screen is recorded in the audit trail."
        breadcrumbs={[list, { label: "Candidate", href: pathOf(candidateId) }]}
        sequence={sequence}
        actions={data && <ReleaseAction specimenId={specimenId} candidate={data} />}
      />
      <ParentIdentity parent={workspace} />
      <RefreshControl query={candidate} />
      <QueryContent query={candidate} what="report candidate">
        {(loaded) => <CandidateReview specimenId={specimenId} candidate={loaded} />}
      </QueryContent>
    </Page>
  );
}

/** Release, when the engine allows it; once released, back to the specimen's workspace. */
function ReleaseAction({
  specimenId,
  candidate,
}: {
  specimenId: string;
  candidate: CandidateView;
}) {
  const navigate = useNavigate();
  const [isReleasing, setReleasing] = useState(false);
  return (
    <>
      {candidate.allowedActions.includes(RELEASE_REPORT) && (
        <Button variant="primary" onPress={() => setReleasing(true)}>
          Release report
        </Button>
      )}
      {isReleasing && (
        <ReleaseDialog
          candidate={candidate}
          onReleased={() => navigate(workspacePath(specimenId))}
          onClose={() => setReleasing(false)}
        />
      )}
    </>
  );
}

function CandidateReview({
  specimenId,
  candidate,
}: {
  specimenId: string;
  candidate: CandidateView;
}) {
  const workspace = workspacePath(specimenId);
  return (
    <div className={styles.stack}>
      <ReleaseBlocked gates={candidate.gates} />
      {candidate.stale && (
        <Alert tone="warning" title="The content changed since this candidate was prepared">
          <p>
            It can no longer be released. <Link to={workspace}>Open the report workspace</Link> to
            prepare a new candidate.
          </p>
          <GateReasons label="What changed" gates={candidate.staleReasons} />
        </Alert>
      )}
      <Section title="Candidate">
        <CandidateSummary candidate={candidate} />
      </Section>
      <Section title="Expected tests">
        <Dispositions dispositions={candidate.dispositions} />
      </Section>
      <Section title="Content as it will print">
        <ReportContent content={candidate.content} />
      </Section>
    </div>
  );
}
