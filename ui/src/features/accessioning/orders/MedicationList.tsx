import { useFieldArray, useFormContext } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, DATE_FORMAT, TextInput } from "@/components/FormFields";
import { MEDICATION_SOURCES } from "./labels";
import type { OrderFormValues } from "./orderValues";

/** Medication statements as received, with who reported them (REQ-ORD-007). */
export function MedicationList() {
  const { control } = useFormContext<OrderFormValues>();
  const medications = useFieldArray({ control, name: "medications" });
  return (
    <FieldList
      legend="Medications"
      emptyText="No medications entered."
      items={medications.fields}
      itemLabel={(index) => `Medication ${index + 1}`}
      addLabel="Add medication"
      onAdd={() =>
        medications.append({
          receivedText: "",
          code: "",
          codeSystem: "",
          source: null,
          reporter: "",
          providedOn: "",
          asOfDate: "",
        })
      }
      onRemove={medications.remove}
      renderItem={(index) => (
        <>
          <TextInput
            name={`medications.${index}.receivedText`}
            label="Medication as written"
            isRequired
          />
          <TextInput name={`medications.${index}.code`} label="Code" />
          <TextInput name={`medications.${index}.codeSystem`} label="Code system" />
          <ChoiceInput
            name={`medications.${index}.source`}
            label="Reported by"
            options={MEDICATION_SOURCES}
            isRequired
          />
          <TextInput name={`medications.${index}.reporter`} label="Reporter's name" />
          <TextInput
            name={`medications.${index}.providedOn`}
            label="Provided on"
            format={DATE_FORMAT}
          />
          <TextInput
            name={`medications.${index}.asOfDate`}
            label="List current as of"
            format={DATE_FORMAT}
          />
        </>
      )}
    />
  );
}
