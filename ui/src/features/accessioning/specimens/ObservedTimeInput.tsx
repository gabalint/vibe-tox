import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";
/** Preserve fractional precision while converting an explicitly supplied offset to UTC. */
export function observedInstant(value: string): string | null {
  const match = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})$/.exec(
    value.trim(),
  );
  if (!match) return null;
  const civil = new Date(`${match[1]}Z`);
  if (!Number.isFinite(civil.getTime()) || civil.toISOString().slice(0, 19) !== match[1])
    return null;
  const date = new Date(`${match[1]}${match[3]}`);
  if (!Number.isFinite(date.getTime())) return null;
  return date.toISOString().replace(/\.\d{3}Z$/, `.${(match[2] ?? "").padEnd(9, "0")}Z`);
}
export function ObservedTimeInput({
  name,
  label,
  required = false,
}: {
  name: string;
  label: string;
  required?: boolean;
}) {
  const { control } = useFormContext();
  return (
    <Controller
      name={name}
      control={control}
      rules={{
        validate: (value: string) =>
          (!value.trim() && !required) ||
          observedInstant(value) !== null ||
          "Enter a valid ISO date/time with seconds and Z or a UTC offset.",
      }}
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          label={label}
          isRequired={required}
          description="As observed, with timezone: YYYY-MM-DDTHH:MM:SS-04:00 or UTC ending Z. No time is assumed."
          autoComplete="off"
        />
      )}
    />
  );
}
