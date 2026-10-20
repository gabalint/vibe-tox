import { CheckList, ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import {
  type ImpactDecision,
  RECORD_IMPACT_ASSESSMENT,
  REPORT_QUERIES,
  type RecordImpactAssessmentInput,
} from "../contract";
import { IMPACT_DECISIONS } from "../labels";

interface ImpactValues {
  keys: string[];
  decision: ImpactDecision | null;
  reason: string;
}

interface ImpactDialogProps {
  revisionId: string;
  open: readonly { key: string; message: string }[];
  onClose: () => void;
}

/**
 * Records a quality decision on what changed after release (REQ-RPT-002). A new change opens
 * the impact again; "correction required" keeps the revision due for correction until a new
 * revision supersedes it.
 */
export function ImpactDialog({ revisionId, open, onClose }: ImpactDialogProps) {
  return (
    <CommandDialog<ImpactValues, RecordImpactAssessmentInput>
      title="Record impact assessment"
      submitLabel="Record assessment"
      command={{ name: RECORD_IMPACT_ASSESSMENT, invalidates: REPORT_QUERIES }}
      defaultValues={{ keys: [], decision: null, reason: "" }}
      toInput={({ keys, decision, reason }) => ({
        revisionId,
        keys,
        decision: checked(decision),
        reason: reason.trim(),
      })}
      successTitle="Impact assessment recorded"
      onClose={onClose}
    >
      <CheckList
        name="keys"
        legend="Changes assessed"
        options={open.map((item) => ({ id: item.key, label: item.message }))}
        requiredMessage="Choose the changes this decision covers."
      />
      <ChoiceInput name="decision" label="Decision" options={IMPACT_DECISIONS} isRequired />
      <ReasonField />
    </CommandDialog>
  );
}
