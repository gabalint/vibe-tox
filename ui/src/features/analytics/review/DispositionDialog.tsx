import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { RecordResultDispositionInput } from "@/generated/RecordResultDispositionInput";
import type { ResultLineView } from "@/generated/ResultLineView";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { RESULT_QUERIES } from "../contract";
import { DISPOSITIONS } from "../resultLabels";

/**
 * Records that a line has no usable source value: invalid, indeterminate, missing or not
 * performed (REQ-RES-001). It is a new revision with a reason; earlier revisions stay.
 */
export function DispositionDialog({
  line,
  onClose,
}: {
  line: ResultLineView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<
      { outcome: string | null; reason: string },
      RecordResultDispositionInput,
      ResultRevisionView
    >
      title={`Record disposition for ${line.analyteName}`}
      submitLabel="Record disposition"
      command={{ name: "record_result_disposition", invalidates: RESULT_QUERIES }}
      defaultValues={{ outcome: null, reason: "" }}
      toInput={(values) => ({
        resultId: line.resultId,
        expectedRevision: line.revision,
        componentId: null,
        analyteId: null,
        specimenId: null,
        outcome: checked(choiceOf(DISPOSITIONS, values.outcome)),
        reason: values.reason.trim(),
      })}
      successTitle={`Disposition recorded for ${line.analyteName}`}
      onClose={onClose}
    >
      <ChoiceInput name="outcome" label="Disposition" options={DISPOSITIONS} isRequired />
      <ReasonField />
    </CommandDialog>
  );
}
