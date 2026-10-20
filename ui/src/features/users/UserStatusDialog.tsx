import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { ChangeUserStatusInput } from "@/generated/ChangeUserStatusInput";
import type { UserSummary } from "@/generated/UserSummary";
import { useCommandMutation } from "@/lib/commands";
import { LIST_USERS } from "./contract";
import { NOT_SAVED_TITLE } from "./UserFormDialog";
import styles from "./users.module.css";

interface UserStatusDialogProps {
  user: UserSummary;
  onClose: () => void;
  onReload: () => void;
}

/**
 * Disables or enables a person's sign-in, with a reason (REQ-IDN-004, REQ-OPS-001). Their
 * records and audit history are kept either way.
 */
export function UserStatusDialog({ user, onClose, onReload }: UserStatusDialogProps) {
  const toast = useToast();
  const disabling = user.status === "active";
  const change = useCommandMutation<UserSummary, { input: ChangeUserStatusInput }>(
    disabling ? "disable_user" : "enable_user",
    { invalidates: [LIST_USERS] },
  );
  const form = useForm<{ reason: string }>({ defaultValues: { reason: "" } });
  const confirm = form.handleSubmit(({ reason }) =>
    change.mutate(
      { input: { userId: user.userId, expectedRevision: user.revision, reason } },
      {
        onSuccess: () => {
          const title = `${user.displayName} was ${disabling ? "disabled" : "enabled"}`;
          toast.show({ tone: "success", title });
          onClose();
        },
      },
    ),
  );
  // `conflict`: changed since loaded, the last active administrator, or already this status.
  const conflict = change.error?.code === "conflict";

  return (
    <ConfirmDialog
      title={`${disabling ? "Disable" : "Enable"} ${user.displayName}?`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={disabling ? "Disable user" : "Enable user"}
      onConfirm={() => void confirm()}
      isPending={change.isPending}
      variant={disabling ? "danger" : "primary"}
    >
      <div className={styles.stack}>
        <p>
          {disabling
            ? `${user.displayName} will no longer be able to sign in. Their records and audit history are kept, and the account can be enabled again.`
            : `${user.displayName} will be able to sign in again with their current password and authenticator app.`}
        </p>
        <FormProvider {...form}>
          <ReasonField />
        </FormProvider>
        {change.error && <ErrorState title={NOT_SAVED_TITLE} error={change.error} />}
        {conflict && <Button onPress={onReload}>Reload users</Button>}
      </div>
    </ConfirmDialog>
  );
}
