import type { FormEvent, ReactNode } from "react";
import { type FieldValues, FormProvider, type UseFormReturn } from "react-hook-form";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { Form } from "@/components/Form";
import type { CommandError } from "@/generated/CommandError";

interface FormDialogProps<Values extends FieldValues> {
  title: string;
  submitLabel: string;
  form: UseFormReturn<Values>;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  /** The command's state: its last error and whether it is running. */
  command: { error: CommandError | null; isPending: boolean };
  /** Heading of the engine's refusal, such as "The change was not saved". */
  errorTitle: string;
  onClose: () => void;
  /** Buttons after the submit button, such as a reload offered after a conflict. */
  extraActions?: ReactNode;
  children: ReactNode;
}

/**
 * A dialog holding one react-hook-form form that runs one engine command. The fields reach
 * the form through `FormProvider`. Escape and outside clicks close it only while nothing is
 * typed and no command runs, so neither typed input nor the command's outcome is lost. Mount
 * it only while it is open.
 */
export function FormDialog<Values extends FieldValues>({
  title,
  submitLabel,
  form,
  onSubmit,
  command,
  errorTitle,
  onClose,
  extraActions,
  children,
}: FormDialogProps<Values>) {
  return (
    <Dialog
      title={title}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      isDismissable={!command.isPending && !form.formState.isDirty}
    >
      <FormProvider {...form}>
        <Form
          aria-label={title}
          onSubmit={onSubmit}
          error={command.error}
          errorTitle={errorTitle}
          actions={
            <>
              <Button type="submit" variant="primary" isPending={command.isPending}>
                {submitLabel}
              </Button>
              {extraActions}
              <Button isDisabled={command.isPending} onPress={onClose}>
                Cancel
              </Button>
            </>
          }
        >
          {children}
        </Form>
      </FormProvider>
    </Dialog>
  );
}
