import { useForm } from "react-hook-form";
import { Facts } from "@/components/Facts";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { CommandError } from "@/generated/CommandError";
import type { MeasuredNotOrderedView } from "@/generated/MeasuredNotOrderedView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import type { UseMeasuredResultInput } from "@/generated/UseMeasuredResultInput";
import { checked } from "@/lib/choices";
import { useCommandMutation } from "@/lib/commands";
import { RESULT_QUERIES } from "../contract";
import { receivedValueText } from "../labels";
import { explainWordOnDilution } from "./wordOnDilution";

/** The engine's refusals of this command, in plain words (F17). */
const REFUSALS: Record<string, string> = {
  other_order:
    "The add-on test belongs to another order, so this specimen's measurement cannot fill it.",
  matrix_mismatch: "This accession has no specimen of the type the add-on test requires.",
  not_served: "The method that measured this specimen does not perform the add-on test for it.",
  nothing_kept: "The run no longer keeps a measurement of the add-on test's analytes.",
  not_equivalent: "The kept measurements of an analyte on this run disagree, so none can be used.",
};

/** The refusal with its field errors said in plain words, since no field here holds them. */
function inPlainWords(error: CommandError | null): CommandError | null {
  const words = error?.fieldErrors.flatMap(({ code }) => REFUSALS[code] ?? []) ?? [];
  return error && words.length > 0 ? { ...error, message: words.join(" ") } : error;
}

interface UseMeasuredResultDialogProps {
  measurement: MeasuredNotOrderedView;
  onClose: () => void;
}

/**
 * Uses a measurement kept outside the ordered scope for an accepted add-on test (F17,
 * `review_results`), with a reason. The engine rechecks every gate.
 */
export function UseMeasuredResultDialog({ measurement, onClose }: UseMeasuredResultDialogProps) {
  const toast = useToast();
  const components = measurement.addOnComponents;
  const only = components.length === 1 ? components[0] : undefined;
  const use = useCommandMutation<SpecimenResults, { input: UseMeasuredResultInput }>(
    "use_measured_result",
    { invalidates: RESULT_QUERIES },
  );
  const form = useForm<{ orderComponentId: string | null; reason: string }>({
    defaultValues: { orderComponentId: only?.componentId ?? null, reason: "" },
  });
  const analyte = `${measurement.analyteName} (${measurement.analyteCode})`;
  const submit = form.handleSubmit((values) => {
    const input = {
      orderComponentId: checked(values.orderComponentId),
      executionId: measurement.executionId,
      reason: values.reason.trim(),
    };
    const test = components.find((component) => component.componentId === input.orderComponentId);
    use.mutate(
      { input },
      {
        onSuccess: () => {
          toast.show({
            tone: "success",
            title: `${analyte} used for ${test?.orderableCode ?? "the add-on test"}; it needs technical review`,
          });
          onClose();
        },
      },
    );
  });
  return (
    <FormDialog
      form={form}
      title={`Use the measured ${analyte}`}
      submitLabel="Use measured result"
      errorTitle="Not used"
      command={{ error: explainWordOnDilution(inPlainWords(use.error)), isPending: use.isPending }}
      onSubmit={submit}
      onClose={onClose}
    >
      <p>
        The measurement becomes the result of the add-on test, recorded with your reason. It then
        goes through technical review and every release gate, including QC for its run.
      </p>
      <Facts items={[{ label: "Measured", value: receivedValueText(measurement) }]} />
      {only ? (
        <Facts items={[{ label: "Add-on test", value: only.orderableCode }]} />
      ) : (
        <ChoiceInput
          name="orderComponentId"
          label="Add-on test"
          options={components.map((component) => ({
            id: component.componentId,
            label: component.orderableCode,
          }))}
          isRequired
        />
      )}
      <ReasonField />
    </FormDialog>
  );
}
