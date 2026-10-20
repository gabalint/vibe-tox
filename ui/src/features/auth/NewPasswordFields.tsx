import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";
import { checkNewPassword, PASSWORD_RULE } from "@/lib/password";

/** The values these fields add to the surrounding form. */
export interface NewPasswordValues {
  newPassword: string;
  confirmPassword: string;
}

interface NewPasswordFieldsProps {
  label?: string;
  /** The engine's input field for the new password, so its field errors show here. */
  engineField?: string;
}

/** A new password and its confirmation, inside a react-hook-form `FormProvider`. */
export function NewPasswordFields({
  label = "New password",
  engineField = "newPassword",
}: NewPasswordFieldsProps) {
  const { control } = useFormContext<NewPasswordValues>();
  return (
    <>
      <Controller
        control={control}
        name="newPassword"
        rules={{ validate: checkNewPassword }}
        render={(controller) => (
          <TextField
            {...textFieldProps(controller)}
            name={engineField}
            label={label}
            type="password"
            autoComplete="new-password"
            description={PASSWORD_RULE}
            isRequired
          />
        )}
      />
      <Controller
        control={control}
        name="confirmPassword"
        rules={{ validate: checkConfirmation }}
        render={(controller) => (
          <TextField
            {...textFieldProps(controller)}
            label={`Confirm ${label.toLowerCase()}`}
            type="password"
            autoComplete="new-password"
            isRequired
          />
        )}
      />
    </>
  );
}

function checkConfirmation(value: string, values: NewPasswordValues): true | string {
  if (value === "") return "Enter the password again.";
  return value === values.newPassword || "The passwords do not match.";
}
