import { useFieldArray, useFormContext } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { TextInput } from "@/components/FormFields";
import type { OrderFormValues } from "./orderValues";

/** Client point-of-care results: attributed context only, never laboratory results (REQ-ORD-007). */
export function PocResultList() {
  const { control } = useFormContext<OrderFormValues>();
  const results = useFieldArray({ control, name: "pointOfCareResults" });
  return (
    <FieldList
      name="pointOfCareResults"
      legend="Point-of-care results"
      emptyText="No point-of-care results entered. They are context only and never become laboratory results."
      items={results.fields}
      itemLabel={(index) => `Point-of-care result ${index + 1}`}
      addLabel="Add point-of-care result"
      onAdd={() =>
        results.append({
          deviceOrTest: "",
          analyteText: "",
          resultText: "",
          performedAtText: "",
          source: "",
        })
      }
      onRemove={results.remove}
      renderItem={(index) => (
        <>
          <TextInput
            name={`pointOfCareResults.${index}.deviceOrTest`}
            label="Device or test"
            isRequired
          />
          <TextInput name={`pointOfCareResults.${index}.analyteText`} label="Drug" isRequired />
          <TextInput
            name={`pointOfCareResults.${index}.resultText`}
            label="Result as written"
            isRequired
          />
          <TextInput
            name={`pointOfCareResults.${index}.performedAtText`}
            label="Performed at, as written"
          />
          <TextInput name={`pointOfCareResults.${index}.source`} label="Reported by" isRequired />
        </>
      )}
    />
  );
}
