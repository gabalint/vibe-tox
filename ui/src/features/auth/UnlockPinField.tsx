import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";

/** Preserve the PIN as a string, including any leading zero; the engine owns its policy. */
export function UnlockPinField({
  name = "pin",
  label = "Unlock PIN",
  confirm = false,
  autoFocus = false,
}: {
  name?: string;
  label?: string;
  confirm?: boolean;
  autoFocus?: boolean;
}) {
  const { control, getValues } = useFormContext();
  return (
    <Controller
      control={control}
      name={name}
      rules={{
        validate: (value: string) => {
          if (!/^[0-9]{6}$/.test(value)) return "Enter exactly six digits.";
          return !confirm || value === getValues("pin") || "The PINs do not match.";
        },
      }}
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          label={label}
          type="password"
          inputMode="numeric"
          autoComplete="off"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          autoFocus={autoFocus}
          description={
            confirm ? "Enter the same six digits again." : "Six digits, including any leading zero."
          }
          isRequired
        />
      )}
    />
  );
}
