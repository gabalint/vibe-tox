import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";
import { checkNewPassword, PASSWORD_RULE } from "@/lib/password";

/**
 * A password the person must replace at their next sign-in, as `temporaryPassword`, inside
 * a react-hook-form `FormProvider` (REQ-IDN-004).
 */
export function TemporaryPasswordField() {
  const { control } = useFormContext<{ temporaryPassword: string }>();
  return (
    <Controller
      control={control}
      name="temporaryPassword"
      rules={{ validate: checkNewPassword }}
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          label="Temporary password"
          type="password"
          autoComplete="new-password"
          description={`${PASSWORD_RULE} Give it to the person privately; they choose their own at their next sign-in.`}
          isRequired
        />
      )}
    />
  );
}
