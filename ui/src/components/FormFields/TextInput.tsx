import { type ReactNode, useContext } from "react";
import { Controller, useFormContext } from "react-hook-form";
import { DatePickerButton } from "@/components/FieldPicker";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";
import { calendarDayToday, LabTimeZoneContext } from "@/lib/format";

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

/** Only the length is checked here; the engine also checks the last (check) digit. */
export const NPI_FORMAT: TextFormat = {
  pattern: /^\d{10}$/,
  message: "The NPI must be exactly 10 digits.",
  description: "10 digits. The last digit is checked automatically.",
};

interface TextInputProps {
  /** The form path, such as `diagnoses.0.code`; also the key for engine field errors. */
  name: string;
  label: string;
  isRequired?: boolean;
  /** Shown but not editable, such as a value the engine no longer lets change. */
  isDisabled?: boolean;
  description?: string;
  format?: TextFormat;
  autoFocus?: boolean;
  /** Additional lexical or range validation; blank handling is owned by isRequired. */
  validate?: (value: string) => true | string;
  /**
   * A picker button inside the field, given the current value and a setter that validates.
   * Date fields get a calendar without one.
   */
  picker?: (value: string, pick: (value: string) => void) => ReactNode;
  /** For a date field: the calendar offers no day after today in the laboratory's timezone. */
  isPastOnly?: boolean;
}

/** A text field bound to the surrounding react-hook-form form. */
export function TextInput({
  name,
  label,
  isRequired,
  isDisabled,
  description,
  format,
  autoFocus,
  validate,
  picker,
  isPastOnly,
}: TextInputProps) {
  const { control, setValue } = useFormContext();
  const labTimeZone = useContext(LabTimeZoneContext);
  const fieldPicker =
    picker ??
    (format === DATE_FORMAT
      ? (_: string, pick: (value: string) => void) => (
          <DatePickerButton
            label={`Choose the ${label.toLowerCase()}`}
            latest={isPastOnly ? calendarDayToday(new Date(), labTimeZone) : undefined}
            onPick={pick}
          />
        )
      : undefined);
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
          isDisabled={isDisabled}
          autoFocus={autoFocus}
          autoComplete="off"
          adornment={
            isDisabled
              ? undefined
              : fieldPicker?.(controller.field.value, (value) =>
                  setValue(name, value, { shouldDirty: true, shouldValidate: true }),
                )
          }
        />
      )}
    />
  );
}
