import { useContext } from "react";
import { FormValidationContext } from "react-aria-components";
import { Controller, useFormContext } from "react-hook-form";
import { CheckboxGroup } from "@/components/CheckboxGroup";

interface CheckListProps {
  /** The form path of a `string[]` of selected IDs. */
  name: string;
  legend: string;
  options: readonly { id: string; label: string; details?: readonly string[] }[];
  /** Makes at least one choice required, with this message when none is made. */
  requiredMessage?: string;
}

/**
 * A group of checkboxes bound to one list in the surrounding react-hook-form form. The
 * engine's error for the list shows under it, as the surrounding `Form` names it.
 */
export function CheckList({ name, legend, options, requiredMessage }: CheckListProps) {
  const { control } = useFormContext();
  const engineError = useContext(FormValidationContext)[name];
  const engineMessage = Array.isArray(engineError) ? engineError.join(" ") : engineError;
  return (
    <Controller
      control={control}
      name={name}
      rules={{
        validate: (value: string[]) =>
          requiredMessage === undefined || value.length > 0 || requiredMessage,
      }}
      render={({ field, fieldState }) => (
        <CheckboxGroup
          legend={legend}
          options={options}
          value={field.value}
          onChange={field.onChange}
          isRequired={requiredMessage !== undefined}
          errorMessage={fieldState.error?.message ?? engineMessage}
        />
      )}
    />
  );
}
