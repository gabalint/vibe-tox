import { FormProvider, useFieldArray, useForm } from "react-hook-form";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { FieldList } from "@/components/FieldList";
import { FieldGrid, Form } from "@/components/Form";
import { DATE_FORMAT, TextInput } from "@/components/FormFields";
import type { CommandError } from "@/generated/CommandError";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import type { PatientDetailsValues } from "./registration";

interface PatientDetailsFormProps {
  defaultValues: PatientDetailsValues;
  /** The details were typed before (back from the check), so leaving loses them. */
  hasDraft: boolean;
  /** The engine's answer when it refused these details, shown on the matching fields. */
  error: CommandError | null;
  onCheck: (values: PatientDetailsValues) => void;
  returnPath?: string;
}

/** Demographics and identifiers exactly as the requisition gives them (REQ-IDN-001). */
export function PatientDetailsForm({
  defaultValues,
  hasDraft,
  error,
  onCheck,
  returnPath = "/patients",
}: PatientDetailsFormProps) {
  const navigate = useNavigate();
  const form = useForm({ defaultValues });
  const identifiers = useFieldArray({ control: form.control, name: "identifiers" });
  return (
    <FormProvider {...form}>
      <UnsavedChangesGuard when={hasDraft || form.formState.isDirty} />
      <Form
        aria-label="Patient details"
        onSubmit={form.handleSubmit(onCheck)}
        error={error}
        errorTitle="These details were not accepted"
        actions={
          <>
            <Button type="submit" variant="primary">
              Check for existing patients
            </Button>
            <Button onPress={() => navigate(returnPath)}>Cancel</Button>
          </>
        }
      >
        <p>
          If the birth date is unknown, partial, or only an age was supplied, retain it in
          Incomplete identity intake. Never invent a complete date.
        </p>
        <Button onPress={() => navigate("/patients/incomplete-intake")}>
          Record incomplete identity intake
        </Button>
        <FieldGrid>
          <TextInput name="familyName" label="Family name" isRequired autoFocus />
          <TextInput name="givenNames" label="Given names" />
          <TextInput name="birthDate" label="Birth date" format={DATE_FORMAT} isRequired />
          <TextInput
            name="administrativeSex"
            label="Administrative sex"
            description="As written on the requisition."
          />
          <TextInput
            name="source"
            label="Source of these details"
            description="Such as the paper requisition."
            isRequired
          />
        </FieldGrid>
        <FieldList
          name="identifiers"
          legend="Identifiers"
          emptyText="No identifiers added."
          items={identifiers.fields}
          itemLabel={(index) => `Identifier ${index + 1}`}
          addLabel="Add identifier"
          onAdd={() =>
            identifiers.append({ assigningAuthority: "", identifierType: "", value: "" })
          }
          onRemove={identifiers.remove}
          renderItem={(index) => (
            <>
              <TextInput name={`identifiers.${index}.value`} label="Identifier" isRequired />
              <TextInput
                name={`identifiers.${index}.assigningAuthority`}
                label="Assigning authority"
                isRequired
              />
              <TextInput
                name={`identifiers.${index}.identifierType`}
                label="Identifier type"
                description="Such as MR"
                isRequired
              />
            </>
          )}
        />
      </Form>
    </FormProvider>
  );
}
