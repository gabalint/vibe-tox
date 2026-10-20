import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { Form } from "@/components/Form";
import { PasswordField } from "@/components/PasswordField";
import { useToast } from "@/components/Toast";
import type { ChangePasswordInput } from "@/generated/ChangePasswordInput";
import { useCommandMutation } from "@/lib/commands";
import { NewPasswordFields, type NewPasswordValues } from "./NewPasswordFields";

interface ChangePasswordValues extends NewPasswordValues {
  password: string;
}

interface ChangePasswordDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

/** The signed-in person changes their own password (REQ-IDN-004). */
export function ChangePasswordDialog({ isOpen, onClose }: ChangePasswordDialogProps) {
  return (
    <Dialog
      title="Change password"
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <ChangePasswordForm onClose={onClose} />
    </Dialog>
  );
}

// Mounted only while the dialog is open, so every opening starts with empty fields.
function ChangePasswordForm({ onClose }: { onClose: () => void }) {
  const toast = useToast();
  const change = useCommandMutation<null, { input: ChangePasswordInput }>("change_password", {
    invalidates: [],
  });
  const form = useForm<ChangePasswordValues>({
    defaultValues: { password: "", newPassword: "", confirmPassword: "" },
  });
  const onSubmit = form.handleSubmit(({ password, newPassword }) =>
    change.mutate(
      { input: { currentPassword: password, newPassword } },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: "Your password was changed" });
          onClose();
        },
        onError: (error) => {
          form.resetField("password");
          // A wrong current password is a field error on `currentPassword`, not a sign-out.
          if (error.fieldErrors.some(({ field }) => field === "currentPassword")) {
            form.setFocus("password");
          }
        },
      },
    ),
  );

  return (
    <FormProvider {...form}>
      <Form
        aria-label="Change password"
        onSubmit={onSubmit}
        error={change.error}
        errorTitle="The password was not changed"
        actions={
          <>
            <Button type="submit" variant="primary" isPending={change.isPending}>
              Change password
            </Button>
            <Button isDisabled={change.isPending} onPress={onClose}>
              Cancel
            </Button>
          </>
        }
      >
        <PasswordField label="Current password" engineField="currentPassword" autoFocus />
        <NewPasswordFields />
      </Form>
    </FormProvider>
  );
}
