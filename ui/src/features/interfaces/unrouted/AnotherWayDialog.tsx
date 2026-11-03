import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { TextInput } from "@/components/FormFields";
import type { CommandError } from "@/generated/CommandError";

interface AnotherWayDialogProps {
  /** Records the delivery with how it was made. */
  onSubmit: (method: string) => void;
  command: { error: CommandError | null; isPending: boolean };
  onClose: () => void;
}

/** Records that an unrouted message was delivered outside the LIS, and how. */
export function AnotherWayDialog({ onSubmit, command, onClose }: AnotherWayDialogProps) {
  const form = useForm({ defaultValues: { method: "" } });
  return (
    <FormDialog
      form={form}
      title="Deliver another way"
      submitLabel="Record delivery"
      errorTitle="Not recorded"
      fieldPaths={{ "decision.method": "method" }}
      command={command}
      onSubmit={form.handleSubmit(({ method }) => onSubmit(method.trim()))}
      onClose={onClose}
    >
      <TextInput
        name="method"
        label="How it was delivered"
        description="For example, faxed to the billing company on 555-0100"
        isRequired
        autoFocus
      />
    </FormDialog>
  );
}
