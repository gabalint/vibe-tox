import { useFormContext, useWatch } from "react-hook-form";
import { Facts } from "@/components/Facts";
import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { RecordReflexDecisionInput } from "@/generated/RecordReflexDecisionInput";
import type { ReflexTriggerView } from "@/generated/ReflexTriggerView";
import type { ResultLineView } from "@/generated/ResultLineView";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import type { TriggerAssessmentView } from "@/generated/TriggerAssessmentView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { ORDER_QUERIES, RESULT_QUERIES } from "../contract";
import {
  classificationText,
  REFLEX_DECISIONS,
  resultValueText,
  SEQUENCE_ROLES,
} from "../resultLabels";

/** `list_reflex_triggers({input:{specimenId}}) → ReflexTriggerView[]` (`review_results`). */
export const LIST_REFLEX_TRIGGERS = "list_reflex_triggers";

interface ReflexValues {
  resultRevisionId: string | null;
  decision: string | null;
  sequenceRole: string | null;
  reason: string;
}

interface Evidence {
  id: string;
  label: string;
  revision: ResultRevisionView;
}

/**
 * Records whether a reflex condition is met (REQ-ORD-003). The reviewer picks one of the
 * reviewed results the engine offers and sees its value, units and cutoff classification
 * read-only; the classification informs the decision but never makes it. "Met" requests the
 * authorized reflex test with the sequence role the reviewer enters; "not met" creates no
 * test, charge or result. Both need a reason and are audited.
 */
export function ReflexDecisionDialog({
  trigger,
  lines,
  onClose,
}: {
  trigger: ReflexTriggerView;
  lines: readonly ResultLineView[];
  onClose: () => void;
}) {
  const evidence: Evidence[] = lines.flatMap((line) =>
    line.current && trigger.evidenceRevisionIds.includes(line.current.revisionId)
      ? [
          {
            id: line.current.revisionId,
            label: `${line.analyteName} (${line.analyteCode})`,
            revision: line.current,
          },
        ]
      : [],
  );
  return (
    <CommandDialog<ReflexValues, RecordReflexDecisionInput, TriggerAssessmentView>
      title={`Reflex decision for ${trigger.orderableName} (${trigger.orderableCode})`}
      submitLabel="Record reflex decision"
      command={{
        name: "record_reflex_decision",
        invalidates: [LIST_REFLEX_TRIGGERS, ...RESULT_QUERIES, ...ORDER_QUERIES],
      }}
      defaultValues={{ resultRevisionId: null, decision: null, sequenceRole: null, reason: "" }}
      toInput={(values) => {
        const decision = checked(choiceOf(REFLEX_DECISIONS, values.decision));
        return {
          parentItemId: trigger.parentItemId,
          orderableId: trigger.orderableId,
          resultRevisionId: checked(choiceOf(evidence, values.resultRevisionId)),
          decision,
          sequenceRole:
            decision === "condition_met"
              ? checked(choiceOf(SEQUENCE_ROLES, values.sequenceRole))
              : null,
          reason: values.reason.trim(),
        };
      }}
      successTitle="Reflex decision recorded"
      onClose={onClose}
    >
      <ChoiceInput
        name="resultRevisionId"
        label="Result the decision rests on"
        options={evidence}
        description="Results with a current approved technical review."
        isRequired
      />
      <SelectedResult evidence={evidence} />
      <ChoiceInput name="decision" label="Reflex condition" options={REFLEX_DECISIONS} isRequired />
      <SequenceRoleChoice />
      <ReasonField />
    </CommandDialog>
  );
}

/** The chosen result exactly as the engine sent it. */
function SelectedResult({ evidence }: { evidence: readonly Evidence[] }) {
  const { control } = useFormContext<ReflexValues>();
  const selected = useWatch({ control, name: "resultRevisionId" });
  const chosen = evidence.find((item) => item.id === selected);
  if (!chosen) return null;
  return (
    <Facts
      items={[
        { label: "Result", value: resultValueText(chosen.revision) },
        { label: "Classification", value: classificationText(chosen.revision) },
        { label: "Revision", value: String(chosen.revision.revisionNo) },
      ]}
    />
  );
}

/** The engine requires a sequence role exactly when the condition is met. */
function SequenceRoleChoice() {
  const { control } = useFormContext<ReflexValues>();
  const decision = useWatch({ control, name: "decision" });
  if (decision !== "condition_met") return null;
  return (
    <ChoiceInput
      name="sequenceRole"
      label="Testing sequence role"
      options={SEQUENCE_ROLES}
      description="Entered by you; never inferred from the test."
      isRequired
    />
  );
}
