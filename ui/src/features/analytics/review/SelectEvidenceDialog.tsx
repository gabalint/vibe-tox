import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { CheckList } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { ResultLineView } from "@/generated/ResultLineView";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import type { SelectResultEvidenceInput } from "@/generated/SelectResultEvidenceInput";
import { useCommandMutation } from "@/lib/commands";
import { RESULT_QUERIES } from "../contract";
import { measurementSource, measurementText } from "../resultLabels";
import { explainWordOnDilution } from "./wordOnDilution";

/**
 * Chooses which measurements are the line's evidence, with a reason (REQ-RES-001). This
 * creates a new revision; no measurement is changed or deleted, and the engine accepts only
 * measurements that report the same value. A refusal of a value word on a dilution run is
 * said in plain words.
 */
export function SelectEvidenceDialog({
  line,
  onClose,
}: {
  line: ResultLineView;
  onClose: () => void;
}) {
  const toast = useToast();
  const select = useCommandMutation<ResultRevisionView, { input: SelectResultEvidenceInput }>(
    "select_result_evidence",
    { invalidates: RESULT_QUERIES },
  );
  const form = useForm<{ measurementIds: string[]; reason: string }>({
    defaultValues: { measurementIds: [], reason: "" },
  });
  const measurements = [...(line.current?.evidence ?? []), ...line.unselectedEvidence].filter(
    (measurement) => !measurement.superseded,
  );
  const options = measurements.map((measurement) => ({
    id: measurement.measurementId,
    label: measurementText(measurement),
    details: [measurementSource(measurement)],
  }));
  const submit = form.handleSubmit((values) => {
    const input = {
      resultId: line.resultId,
      expectedRevision: line.revision,
      measurementIds: values.measurementIds,
      reason: values.reason.trim(),
    };
    select.mutate(
      { input },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: `Evidence selected for ${line.analyteName}` });
          onClose();
        },
      },
    );
  });
  return (
    <FormDialog
      form={form}
      title={`Select evidence for ${line.analyteName}`}
      submitLabel="Select evidence"
      errorTitle="Not recorded"
      command={{ error: explainWordOnDilution(select.error), isPending: select.isPending }}
      onSubmit={submit}
      onClose={onClose}
    >
      <CheckList
        name="measurementIds"
        legend="Measurements"
        options={options}
        requiredMessage="Choose at least one measurement."
      />
      <ReasonField />
    </FormDialog>
  );
}
