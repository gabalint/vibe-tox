import { TextInput } from "@/components/FormFields";
import type { ConditionReviewSource } from "@/generated/ConditionReviewSource";
import type { PrepareReportCandidateInput } from "@/generated/PrepareReportCandidateInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { omittedIfBlank } from "@/lib/choices";
import {
  CANDIDATE_QUERIES,
  type CandidateView,
  PREPARE_REPORT_CANDIDATE,
  type ReportRelationship,
} from "../contract";
import { ConditionReviewFields } from "./ConditionReviewFields";

interface PrepareCandidateDialogProps {
  specimenId: string;
  /** What the engine's preview says this revision would be. */
  relationship: ReportRelationship | null;
  conditionSource: ConditionReviewSource;
  hasIssuedReport: boolean;
  onPrepared: (candidate: CandidateView) => void;
  onClose: () => void;
}

/**
 * Freezes the current content as a candidate and renders its PDF (REQ-RPT-001). A correction
 * needs its reason, which is printed on the report (REQ-RPT-002); the engine decides whether
 * the revision is a correction and checks again.
 */
export function PrepareCandidateDialog({
  specimenId,
  relationship,
  conditionSource,
  hasIssuedReport,
  onPrepared,
  onClose,
}: PrepareCandidateDialogProps) {
  const hasConditions = conditionSource.conditions.length > 0;
  const isCorrection = relationship === "correction";
  return (
    <CommandDialog<
      { amendmentReason: string; conditionReview: { reason: string; choices: string[] } },
      PrepareReportCandidateInput,
      CandidateView
    >
      title="Prepare report candidate"
      submitLabel="Prepare candidate"
      command={{ name: PREPARE_REPORT_CANDIDATE, invalidates: CANDIDATE_QUERIES }}
      defaultValues={{
        amendmentReason: "",
        conditionReview: { reason: "", choices: conditionSource.conditions.map(() => "") },
      }}
      toInput={({ amendmentReason, conditionReview }) => ({
        specimenId,
        amendmentReason: omittedIfBlank(amendmentReason),
        conditionReview: hasConditions
          ? {
              sourceSha256: conditionSource.sourceSha256,
              reason: conditionReview.reason,
              decisions: conditionSource.conditions.map((condition, index) => ({
                decisionId: condition.decisionId,
                code: condition.code,
                includeInReport: conditionReview.choices[index] === "include",
              })),
            }
          : undefined,
      })}
      successTitle="Candidate prepared"
      onDone={onPrepared}
      onClose={onClose}
    >
      <p>
        The content shown now is frozen and rendered for your review. It is not released until you
        release it, and it expires after an hour. Preparing replaces any open candidate.
      </p>
      {hasConditions && <ConditionReviewFields source={conditionSource} />}
      {(isCorrection || (hasConditions && hasIssuedReport)) && (
        <TextInput
          name="amendmentReason"
          label="Reason for correction"
          description={
            hasConditions
              ? "Required if your selections change previously reported content. Printed on a correction; leave blank for an unchanged context or supplement. The engine checks the final relationship."
              : "Printed on the corrected report and recorded in the audit trail."
          }
          isRequired={isCorrection && !hasConditions}
        />
      )}
    </CommandDialog>
  );
}
