import { useState } from "react";
import { Button } from "@/components/Button";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import type { ReviewView } from "@/generated/ReviewView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import type { SubmitTechnicalReviewInput } from "@/generated/SubmitTechnicalReviewInput";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { RESULT_QUERIES } from "../contract";
import { resultValueText } from "../resultLabels";
import { analyteText } from "./resultColumns";
import review from "./review.module.css";

const SUBMIT = "submit_technical_review";

interface ShownRevision {
  analyte: string;
  revision: ResultRevisionView;
  approved: boolean;
}

/**
 * Approves or rejects the exact current revisions shown (REQ-RES-005). Approval covers only
 * revisions without a current approval; the engine refuses a repeat approval. When every
 * revision is approved, Approve is replaced by a note and Reject becomes Withdraw approval.
 * If anything changed since the screen loaded, or a gate blocks review, the engine refuses and
 * lists why. Technical review is not release.
 */
export function TechnicalReviewActions({ results }: { results: SpecimenResults }) {
  const [dialog, setDialog] = useState<"approve" | "reject" | null>(null);
  const close = () => setDialog(null);
  const shown: ShownRevision[] = results.lines.flatMap((line) =>
    line.current
      ? [
          {
            analyte: analyteText(line),
            revision: line.current,
            approved: line.reviewStatus.status === "current",
          },
        ]
      : [],
  );
  const unapproved = shown.filter((item) => !item.approved);
  const allApproved = shown.length > 0 && unapproved.length === 0;
  const input = (
    decision: SubmitTechnicalReviewInput["decision"],
    reason: string | null,
    items: ShownRevision[] = shown,
  ) => ({
    specimenId: results.specimenId,
    decision,
    resultRevisionIds: items.map((item) => item.revision.revisionId),
    reason,
  });
  return (
    <>
      {allApproved ? (
        <span className={`${styles.hint} ${review.headerNote}`}>
          All current results are approved. Nothing new to review.
        </span>
      ) : (
        <Button
          variant="primary"
          isDisabled={unapproved.length === 0}
          onPress={() => setDialog("approve")}
        >
          Approve results
        </Button>
      )}
      <Button isDisabled={shown.length === 0} onPress={() => setDialog("reject")}>
        {allApproved ? "Withdraw approval" : "Reject results"}
      </Button>
      {dialog === "approve" && (
        <CommandConfirmDialog<SubmitTechnicalReviewInput, ReviewView>
          title="Approve these results"
          confirmLabel="Approve results"
          variant="primary"
          command={{ name: SUBMIT, invalidates: RESULT_QUERIES }}
          input={input("approved", null, unapproved)}
          successTitle="Technical review recorded"
          onClose={close}
        >
          <p>You approve exactly these revisions. Release is a separate step.</p>
          <RevisionList shown={unapproved} />
          {unapproved.length < shown.length && (
            <p>Results that already have a current approval are not approved again.</p>
          )}
        </CommandConfirmDialog>
      )}
      {dialog === "reject" && (
        <RejectDialog shown={shown} withdraw={allApproved} input={input} onClose={close} />
      )}
    </>
  );
}

interface RejectDialogProps {
  shown: ShownRevision[];
  withdraw: boolean;
  input: (decision: "rejected", reason: string) => SubmitTechnicalReviewInput;
  onClose: () => void;
}

function RejectDialog({ shown, withdraw, input, onClose }: RejectDialogProps) {
  const reject = useCommandMutation<ReviewView, { input: SubmitTechnicalReviewInput }>(SUBMIT, {
    invalidates: RESULT_QUERIES,
  });
  return (
    <ReasonConfirmDialog<SubmitTechnicalReviewInput>
      title={withdraw ? "Withdraw approval of these results" : "Reject these results"}
      confirmLabel={withdraw ? "Withdraw approval" : "Reject results"}
      variant="danger"
      command={reject}
      input={(reason) => input("rejected", reason)}
      successTitle="Technical review recorded"
      onClose={onClose}
    >
      <p>
        {withdraw
          ? "You withdraw approval of exactly these revisions:"
          : "You reject exactly these revisions:"}
      </p>
      <RevisionList shown={shown} />
      {withdraw && (
        <p>
          The results go back to review and cannot be released until they are approved again. A
          report already released is not changed.
        </p>
      )}
    </ReasonConfirmDialog>
  );
}

function RevisionList({ shown }: { shown: ShownRevision[] }) {
  return (
    <ul aria-label="Revisions reviewed" className={styles.list}>
      {shown.map(({ analyte, revision }) => (
        <li key={revision.revisionId}>
          {analyte}: {resultValueText(revision)}, revision {revision.revisionNo}
        </li>
      ))}
    </ul>
  );
}
