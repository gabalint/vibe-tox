import { Controller, useFormContext } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";

/**
 * Why a change is made, as `reason`, inside a react-hook-form `FormProvider`. The engine
 * records it in the audit trail (REQ-OPS-001).
 */
export function ReasonField() {
  const { control } = useFormContext<{ reason: string }>();
  return (
    <Controller
      control={control}
      name="reason"
      rules={{ validate: (value) => value.trim() !== "" || "Enter the reason for this change." }}
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          label="Reason"
          description="Recorded in the audit trail."
          isRequired
        />
      )}
    />
  );
}
