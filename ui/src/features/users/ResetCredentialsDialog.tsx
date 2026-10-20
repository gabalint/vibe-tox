import { Controller, useForm } from "react-hook-form";
import { Checkbox } from "@/components/Checkbox";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { ResetUserCredentialsInput } from "@/generated/ResetUserCredentialsInput";
import type { UserSummary } from "@/generated/UserSummary";
import { useCommandMutation } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { LIST_USERS } from "./contract";
import { TemporaryPasswordField } from "./TemporaryPasswordField";
import { UserFormDialog } from "./UserFormDialog";
import styles from "./users.module.css";

type ResetValues = Omit<ResetUserCredentialsInput, "userId" | "expectedRevision">;

interface ResetCredentialsDialogProps {
  user: UserSummary;
  onClose: () => void;
  onReload: () => void;
}

/**
 * Sets a temporary password, and optionally removes the authenticator; the person's open sessions
 * end. The administrator confirms with their own sign-in method (REQ-IDN-004).
 */
export function ResetCredentialsDialog({ user, onClose, onReload }: ResetCredentialsDialogProps) {
  const toast = useToast();
  const reset = useCommandMutation<UserSummary, { input: ResetUserCredentialsInput }>(
    "reset_user_credentials",
    { invalidates: [LIST_USERS] },
  );
  const form = useForm<ResetValues>({
    defaultValues: { temporaryPassword: "", resetMfa: false, reason: "", password: "" },
  });
  const onSubmit = form.handleSubmit((values) =>
    reset.mutate(
      { input: { ...values, userId: user.userId, expectedRevision: user.revision } },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: `Sign-in reset for ${user.displayName}` });
          onClose();
        },
      },
    ),
  );

  return (
    <UserFormDialog
      title={`Reset sign-in of ${user.displayName}`}
      submitLabel="Reset sign-in"
      form={form}
      onSubmit={onSubmit}
      command={reset}
      onClose={onClose}
      onReload={onReload}
    >
      <TemporaryPasswordField />
      <Controller
        control={form.control}
        name="resetMfa"
        render={({ field }) => (
          <div className={styles.choice}>
            <Checkbox isSelected={field.value} onChange={field.onChange}>
              Also remove the authenticator app
            </Checkbox>
            <p className={styles.choiceDetail}>
              For a lost or replaced device. They set up a new one at their next sign-in.
            </p>
          </div>
        )}
      />
      <ReasonField />
      <AuthenticationField />
    </UserFormDialog>
  );
}
