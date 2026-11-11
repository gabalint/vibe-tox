import { useFieldArray, useFormContext } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { YES_NO } from "@/lib/choices";
import type { OrderFormValues } from "./orderValues";

/** Diagnoses with the document each is written on (REQ-ORD-005). */
export function DiagnosisList() {
  const { control } = useFormContext<OrderFormValues>();
  const diagnoses = useFieldArray({ control, name: "diagnoses" });
  return (
    <FieldList
      name="diagnoses"
      legend="Diagnoses"
      emptyText="No diagnoses entered."
      items={diagnoses.fields}
      itemLabel={(index) => `Diagnosis ${index + 1}`}
      addLabel="Add diagnosis"
      onAdd={() =>
        diagnoses.append({
          code: "",
          codeSystem: "",
          codeSystemVersion: "",
          documentRef: "",
          preprinted: null,
        })
      }
      onRemove={diagnoses.remove}
      renderItem={(index) => (
        <>
          <TextInput name={`diagnoses.${index}.code`} label="Code" isRequired />
          <TextInput
            name={`diagnoses.${index}.codeSystem`}
            label="Code system"
            description="Such as ICD-10-CM"
            isRequired
          />
          <TextInput name={`diagnoses.${index}.codeSystemVersion`} label="Code system version" />
          <TextInput
            name={`diagnoses.${index}.documentRef`}
            label="Source document"
            description="Where the diagnosis is written."
            isRequired
          />
          <ChoiceInput
            name={`diagnoses.${index}.preprinted`}
            label="Pre-printed on the laboratory's form"
            options={YES_NO}
            isRequired
          />
        </>
      )}
    />
  );
}
