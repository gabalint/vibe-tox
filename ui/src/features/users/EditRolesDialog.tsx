import { useForm } from "react-hook-form";
import { FieldGrid, FieldSection } from "@/components/Form";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { RoleCode } from "@/generated/RoleCode";
import type { RoleInfo } from "@/generated/RoleInfo";
import type { SetUserRolesInput } from "@/generated/SetUserRolesInput";
import type { UserSummary } from "@/generated/UserSummary";
import { useCommandMutation } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { LIST_USERS } from "./contract";
import { RoleChecklist } from "./RoleChecklist";
import { UserFormDialog } from "./UserFormDialog";

interface EditRolesDialogProps {
  user: UserSummary;
  roles: readonly RoleInfo[];
  onClose: () => void;
  onReload: () => void;
}

/**
 * Changes what a person may do, with a reason for the audit trail, confirmed with the signed-in
 * person's own password (REQ-IDN-004, REQ-OPS-001).
 */
export function EditRolesDialog({ user, roles, onClose, onReload }: EditRolesDialogProps) {
  const toast = useToast();
  const change = useCommandMutation<UserSummary, { input: SetUserRolesInput }>("set_user_roles", {
    invalidates: [LIST_USERS],
  });
  const form = useForm<{ roles: RoleCode[]; reason: string; password: string }>({
    defaultValues: { roles: user.roles, reason: "", password: "" },
  });
  const onSubmit = form.handleSubmit(({ roles: selected, reason, password }) =>
    change.mutate(
      {
        input: {
          userId: user.userId,
          roles: selected,
          expectedRevision: user.revision,
          reason,
          password,
        },
      },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: `Roles changed for ${user.displayName}` });
          onClose();
        },
      },
    ),
  );

  return (
    <UserFormDialog
      title={`Edit roles of ${user.displayName}`}
      submitLabel="Save roles"
      form={form}
      onSubmit={onSubmit}
      command={change}
      onClose={onClose}
      onReload={onReload}
      size="wide"
    >
      <RoleChecklist roles={roles} />
      <FieldSection title="Reason and confirmation">
        <FieldGrid>
          <ReasonField />
          <AuthenticationField />
        </FieldGrid>
      </FieldSection>
    </UserFormDialog>
  );
}
