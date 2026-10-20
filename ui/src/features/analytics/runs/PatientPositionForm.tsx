import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { TextInput } from "@/components/FormFields";
import { useToast } from "@/components/Toast";
import type { AddPatientPositionInput } from "@/generated/AddPatientPositionInput";
import type { RunDetail } from "@/generated/RunDetail";
import { omittedIfBlank } from "@/lib/choices";
import { useCommandMutation } from "@/lib/commands";
import { RUN_QUERIES } from "../contract";
import { PrepBatchChoice } from "./PrepBatchChoice";
import runs from "./runs.module.css";

/**
 * Places patient material on the run by scanning its container label (REQ-ANA-001). A
 * scanner types the label and presses Enter, which submits; the fields then clear for the
 * next sample. Every addition is an initial attempt. The engine rechecks the specimen, its
 * holds and the order before accepting it, and lists what blocks it. For add-on tests the
 * analyst attests the stability check; the engine does not compute stability. It sits in one
 * row above the positions, so the grid stays within the first screen.
 */
export function PatientPositionForm({ run }: { run: RunDetail }) {
  const toast = useToast();
  const add = useCommandMutation<RunDetail, { input: AddPatientPositionInput }>(
    "add_patient_position",
    { invalidates: RUN_QUERIES },
  );
  const form = useForm({
    defaultValues: {
      positionLabel: "",
      containerLabel: "",
      reason: "",
      prepBatchId: null as string | null,
    },
  });
  const submit = form.handleSubmit((values) =>
    add.mutate(
      {
        input: {
          runId: run.runId,
          positionLabel: values.positionLabel.trim(),
          containerLabel: values.containerLabel.trim(),
          attemptKind: "initial",
          prepBatchId: values.prepBatchId ?? undefined,
          reason: omittedIfBlank(values.reason),
        },
      },
      {
        onSuccess: () => {
          toast.show({
            tone: "success",
            title: `${values.containerLabel.trim()} placed at ${values.positionLabel.trim()}`,
          });
          // Keeping the field references lets focus return to the first field at once.
          form.reset(undefined, { keepFieldsRef: true });
          form.setFocus("positionLabel");
        },
      },
    ),
  );
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Add patient sample"
        onSubmit={submit}
        error={add.error}
        errorTitle="The sample was not placed"
        actions={
          <Button type="submit" variant="primary" isPending={add.isPending}>
            Add sample
          </Button>
        }
      >
        <div className={runs.scanRow}>
          <TextInput name="positionLabel" label="Position" isRequired />
          <TextInput
            name="containerLabel"
            label="Container barcode"
            description="Scan the label; Enter adds the sample."
            isRequired
          />
          <TextInput
            name="reason"
            label="Add-on stability check"
            description="Add-on tests only: how stability was checked."
          />
        </div>
        <PrepBatchChoice methodVersionId={run.methodVersionId} />
      </Form>
    </FormProvider>
  );
}
