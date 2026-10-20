import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";

/** A presence or format check; the engine validates everything again. */
export interface TextFormat {
  pattern: RegExp;
  message: string;
  description: string;
}

export const DATE_FORMAT: TextFormat = {
  pattern: /^\d{4}-\d{2}-\d{2}$/,
  message: "Enter the date as YYYY-MM-DD.",
  description: "YYYY-MM-DD",
};

interface TextInputProps {
  /** The form path, such as `diagnoses.0.code`; also the key for engine field errors. */
  name: string;
  label: string;
  isRequired?: boolean;
  description?: string;
  format?: TextFormat;
  autoFocus?: boolean;
  /** Additional lexical or range validation; blank handling is owned by isRequired. */
  validate?: (value: string) => true | string;
}

/** A text field bound to the surrounding react-hook-form form. */
export function TextInput({
  name,
  label,
  isRequired,
  description,
  format,
  autoFocus,
  validate,
}: TextInputProps) {
  const { control } = useFormContext();
  return (
    <Controller
      control={control}
      name={name}
      rules={{
        validate: (value: string) => {
          if (value.trim() === "") return isRequired ? `Enter ${label.toLowerCase()}.` : true;
          if (format && !format.pattern.test(value.trim())) return format.message;
          return validate?.(value.trim()) ?? true;
        },
      }}
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          label={label}
          description={description ?? format?.description}
          isRequired={isRequired}
          autoFocus={autoFocus}
          autoComplete="off"
        />
      )}
    />
  );
}
