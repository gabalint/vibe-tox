import { useParams } from "react-router";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import { GET_SPECIMEN_RESULTS } from "../contract";
import { REVIEW_LIST, specimenResultsPath } from "../paths";
import { SPECIMEN_ACCEPTANCE } from "../resultLabels";
import { MeasuredNotOrdered } from "./MeasuredNotOrdered";
import { ReflexSection } from "./ReflexSection";
import { ResultLines } from "./ResultLines";
import review from "./review.module.css";
import { TechnicalReviewActions } from "./TechnicalReviewActions";
import { ValiditySection } from "./ValiditySection";

/**
 * One specimen's results for technical review (REQ-RES-001, REQ-RES-005): one grid row per
 * line with its current value, what blocks review and whether an earlier review is out of
 * date, and the open line's history and unselected evidence beside it. The header, with
 * Approve and Reject and the patient and specimen identity (REQ-IDN-001), stays in view; a
 * review approves the exact revisions shown. Opening this screen is recorded in the audit
 * trail. Opened from the review queue, it steps through the queue with Previous and Next.
 */
export function SpecimenResultsPage() {
  const { specimenId = "" } = useParams();
  const results = useCommandQuery<SpecimenResults>(GET_SPECIMEN_RESULTS, {
    input: { specimenId },
  });
  const { list, sequence } = useRecordSequence(specimenId, REVIEW_LIST, specimenResultsPath);
  const data = results.data;
  return (
    <Page>
      <div className={review.header}>
        <PageHeader
          title={data ? `Results for accession ${data.accessionNumber}` : "Specimen results"}
          breadcrumbs={[
            list,
            {
              label: data?.accessionNumber ?? "Specimen results",
              href: specimenResultsPath(specimenId),
            },
          ]}
          sequence={sequence}
          actions={
            <>
              <RefreshControl query={results} />
              {data?.allowedActions.includes("submit_technical_review") && (
                <TechnicalReviewActions results={data} />
              )}
            </>
          }
        />
        {data && (
          // The engine sends no container label code with the results yet.
          <IdentityBanner
            subject={data.subject}
            specimen={{ accessionNumber: data.accessionNumber }}
          />
        )}
      </div>
      <QueryContent query={results} what="specimen results">
        {(loaded) => <SpecimenReview results={loaded} />}
      </QueryContent>
    </Page>
  );
}

function SpecimenReview({ results }: { results: SpecimenResults }) {
  return (
    <>
      <Facts
        layout="compact"
        items={[
          { label: "Specimen type", value: results.specimenTypeCode },
          { label: "Acceptance", value: labelOf(SPECIMEN_ACCEPTANCE, results.acceptance) },
        ]}
      />
      <Section title="Results">
        <ResultLines results={results} />
      </Section>
      <MeasuredNotOrdered results={results} />
      <ReflexSection results={results} />
      <ValiditySection results={results} />
    </>
  );
}
