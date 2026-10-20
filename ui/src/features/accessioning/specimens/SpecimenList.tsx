import { useFieldArray, useFormContext } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import { type AccessionFormValues, EMPTY_SPECIMEN } from "./accessionValues";
import { COUNT_FORMAT, VOLUME_FORMAT } from "./formats";
import { VOLUME_KINDS } from "./labels";

/** The specimens received, each with its containers (REQ-SPC-001). */
export function SpecimenList({ specimenTypes }: { specimenTypes: readonly SpecimenTypeView[] }) {
  const { control } = useFormContext<AccessionFormValues>();
  const specimens = useFieldArray({ control, name: "specimens" });
  const typeOptions = specimenTypes.map((type) => ({ id: type.code, label: type.name }));
  return (
    <FieldList
      legend="Specimens received"
      emptyText="Add each specimen received."
      items={specimens.fields}
      itemLabel={(index) => `Specimen ${index + 1}`}
      addLabel="Add specimen"
      onAdd={() => specimens.append(EMPTY_SPECIMEN)}
      onRemove={specimens.remove}
      renderItem={(index) => (
        <>
          <ChoiceInput
            name={`specimens.${index}.specimenTypeCode`}
            label="Specimen type"
            options={typeOptions}
            isRequired
          />
          <TextInput
            name={`specimens.${index}.containerCount`}
            label="Containers"
            format={COUNT_FORMAT}
            isRequired
          />
          <TextInput name={`specimens.${index}.volume`} label="Volume" format={VOLUME_FORMAT} />
          <TextInput
            name={`specimens.${index}.volumeUnit`}
            label="Volume unit"
            description="Such as mL"
          />
          <ChoiceInput
            name={`specimens.${index}.volumeKind`}
            label="Volume is"
            options={VOLUME_KINDS}
            isRequired
          />
        </>
      )}
    />
  );
}
