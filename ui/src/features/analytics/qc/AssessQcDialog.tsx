import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { AssessQcInput } from "@/generated/AssessQcInput";
import type { QcAssessmentView } from "@/generated/QcAssessmentView";
import type { QcObservationView } from "@/generated/QcObservationView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { QC_QUERIES } from "../contract";
import { QC_OUTCOMES, qcOutcomeStatus } from "../labels";

/** What is assessed: one analyte on the run, the run as a whole, or a preparation batch. */
export type QcTarget =
  | { scope: "run_analyte"; runId: string; analyteId: string }
  | { scope: "run"; runId: string }
  | { scope: "prep_batch"; prepBatchId: string };

const SCOPE_TEXT: Record<Exclude<QcTarget["scope"], "run_analyte">, string> = {
  run: "Assesses the run as a whole.",
  prep_batch: "Assesses the preparation batch. Only a completed batch can be accepted.",
};

interface AssessQcDialogProps {
  title: string;
  target: QcTarget;
  /** The observations the assessment cites, as shown. */
  evidence: QcObservationView[];
  /** The assessment this one replaces, if any. */
  current: QcAssessmentView | null;
  onClose: () => void;
}

/**
 * Accepts or rejects QC (REQ-ANA-004, REQ-ANA-005). The assessment cites the observations
 * shown; if more arrive meanwhile the engine refuses it, so nothing unseen is accepted.
 * Replacing an assessment needs a reason, and there is no waiver.
 */
export function AssessQcDialog({ title, target, evidence, current, onClose }: AssessQcDialogProps) {
  const replaces = current ? qcOutcomeStatus(current.outcome).label.toLowerCase() : null;
  return (
    <CommandDialog<{ outcome: string | null; reason: string }, AssessQcInput>
      title={`Assess ${title}`}
      submitLabel="Record assessment"
      command={{ name: "assess_qc", invalidates: QC_QUERIES }}
      defaultValues={{ outcome: null, reason: "" }}
      toInput={(values) => ({
        ...target,
        outcome: checked(choiceOf(QC_OUTCOMES, values.outcome)),
        evidenceObservationIds: evidence.map((observation) => observation.observationId),
        supersedesAssessmentId: current?.assessmentId,
        reason: omittedIfBlank(values.reason),
      })}
      successTitle="QC assessment recorded"
      onClose={onClose}
    >
      <p>
        {target.scope === "run_analyte"
          ? `Cites the ${evidence.length} current ${evidence.length === 1 ? "observation" : "observations"} shown.`
          : SCOPE_TEXT[target.scope]}
        {replaces && ` Replaces the current assessment (${replaces}).`}
      </p>
      <ChoiceInput name="outcome" label="Decision" options={QC_OUTCOMES} isRequired />
      <TextInput
        name="reason"
        label="Reason"
        description={
          current ? "Recorded in the audit trail." : "Optional for a first assessment; audited."
        }
        isRequired={current !== null}
      />
    </CommandDialog>
  );
}
