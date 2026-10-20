import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";

interface PasswordFieldProps {
  label?: string;
  /** The engine's input field, so its field errors show here. */
  engineField?: string;
  autoFocus?: boolean;
}

/** The person's current password as `password`, inside a react-hook-form `FormProvider`. */
export function PasswordField({
  label = "Password",
  engineField = "password",
  autoFocus,
}: PasswordFieldProps) {
  const { control } = useFormContext<{ password: string }>();
  return (
    <Controller
      control={control}
      name="password"
      rules={{ required: "Enter your password." }}
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          name={engineField}
          label={label}
          type="password"
          autoComplete="current-password"
          isRequired
          autoFocus={autoFocus}
        />
      )}
    />
  );
}
