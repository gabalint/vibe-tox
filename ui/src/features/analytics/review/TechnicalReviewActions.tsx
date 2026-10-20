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

const SUBMIT = "submit_technical_review";

interface ShownRevision {
  analyte: string;
  revision: ResultRevisionView;
}

/**
 * Approves or rejects the exact current revisions shown (REQ-RES-005). If anything changed
 * since the screen loaded, or a gate blocks review, the engine refuses and lists why.
 * Technical review is not release.
 */
export function TechnicalReviewActions({ results }: { results: SpecimenResults }) {
  const [dialog, setDialog] = useState<"approve" | "reject" | null>(null);
  const close = () => setDialog(null);
  const shown: ShownRevision[] = results.lines.flatMap((line) =>
    line.current ? [{ analyte: analyteText(line), revision: line.current }] : [],
  );
  const input = (decision: SubmitTechnicalReviewInput["decision"], reason: string | null) => ({
    specimenId: results.specimenId,
    decision,
    resultRevisionIds: shown.map((item) => item.revision.revisionId),
    reason,
  });
  return (
    <>
      <Button
        variant="primary"
        isDisabled={shown.length === 0}
        onPress={() => setDialog("approve")}
      >
        Approve results
      </Button>
      <Button isDisabled={shown.length === 0} onPress={() => setDialog("reject")}>
        Reject results
      </Button>
      {dialog === "approve" && (
        <CommandConfirmDialog<SubmitTechnicalReviewInput, ReviewView>
          title="Approve these results"
          confirmLabel="Approve results"
          variant="primary"
          command={{ name: SUBMIT, invalidates: RESULT_QUERIES }}
          input={input("approved", null)}
          successTitle="Technical review recorded"
          onClose={close}
        >
          <p>You approve exactly these revisions. Release is a separate step.</p>
          <RevisionList shown={shown} />
        </CommandConfirmDialog>
      )}
      {dialog === "reject" && <RejectDialog shown={shown} input={input} onClose={close} />}
    </>
  );
}

interface RejectDialogProps {
  shown: ShownRevision[];
  input: (decision: "rejected", reason: string) => SubmitTechnicalReviewInput;
  onClose: () => void;
}

function RejectDialog({ shown, input, onClose }: RejectDialogProps) {
  const reject = useCommandMutation<ReviewView, { input: SubmitTechnicalReviewInput }>(SUBMIT, {
    invalidates: RESULT_QUERIES,
  });
  return (
    <ReasonConfirmDialog<SubmitTechnicalReviewInput>
      title="Reject these results"
      confirmLabel="Reject results"
      variant="danger"
      command={reject}
      input={(reason) => input("rejected", reason)}
      successTitle="Technical review recorded"
      onClose={onClose}
    >
      <p>You reject exactly these revisions:</p>
      <RevisionList shown={shown} />
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
