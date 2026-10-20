import { Controller, useFormContext } from "react-hook-form";
import { Select } from "@/components/Select";

interface ChoiceInputProps {
  /** The form path; also the key for engine field errors. */
  name: string;
  label: string;
  options: readonly { id: string; label: string }[];
  isRequired?: boolean;
  /** Nothing to choose yet, such as a location before its client is chosen. */
  isDisabled?: boolean;
  description?: string;
  /** Called after the form value changes, such as to clear fields that depend on it. */
  onChange?: (value: string | null) => void;
}

/**
 * A choice bound to the surrounding react-hook-form form. It starts empty: nothing the
 * requisition states is preselected (REQ-ORD-005).
 */
export function ChoiceInput({
  name,
  label,
  options,
  isRequired,
  isDisabled,
  description,
  onChange,
}: ChoiceInputProps) {
  const { control } = useFormContext();
  return (
    <Controller
      control={control}
      name={name}
      rules={{ required: isRequired ? `Choose ${label.toLowerCase()}.` : false }}
      render={({ field, fieldState }) => (
        <Select
          label={label}
          name={field.name}
          inputRef={field.ref}
          options={options}
          value={field.value}
          onChange={(value) => {
            field.onChange(value);
            onChange?.(value);
          }}
          isRequired={isRequired}
          isDisabled={isDisabled}
          description={description}
          errorMessage={fieldState.error?.message}
        />
      )}
    />
  );
}
