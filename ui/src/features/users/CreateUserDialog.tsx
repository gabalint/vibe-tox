import { Controller, useForm } from "react-hook-form";
import { TextField } from "@/components/TextField";
import { useToast } from "@/components/Toast";
import type { CreateUserInput } from "@/generated/CreateUserInput";
import type { RoleInfo } from "@/generated/RoleInfo";
import type { UserSummary } from "@/generated/UserSummary";
import { useCommandMutation } from "@/lib/commands";
import { textFieldProps } from "@/lib/form";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { LIST_USERS } from "./contract";
import { RoleChecklist } from "./RoleChecklist";
import { TemporaryPasswordField } from "./TemporaryPasswordField";
import { UserFormDialog } from "./UserFormDialog";

interface CreateUserDialogProps {
  roles: readonly RoleInfo[];
  onCreated: (user: UserSummary) => void;
  onClose: () => void;
}

/**
 * Adds a person with a temporary password; they set up MFA at first sign-in. The administrator
 * confirms with their own password (REQ-IDN-004).
 */
export function CreateUserDialog({ roles, onCreated, onClose }: CreateUserDialogProps) {
  const toast = useToast();
  const create = useCommandMutation<UserSummary, { input: CreateUserInput }>("create_user", {
    invalidates: [LIST_USERS],
  });
  const form = useForm<CreateUserInput>({
    defaultValues: {
      username: "",
      displayName: "",
      roles: [],
      temporaryPassword: "",
      password: "",
    },
  });
  const onSubmit = form.handleSubmit((input) =>
    create.mutate(
      { input },
      {
        onSuccess: (user) => {
          toast.show({
            tone: "success",
            title: `${user.displayName} was added`,
            description: "At first sign-in they choose a password and set up an authenticator app.",
          });
          onCreated(user);
          onClose();
        },
      },
    ),
  );

  return (
    <UserFormDialog
      title="Create user"
      submitLabel="Create user"
      form={form}
      onSubmit={onSubmit}
      command={create}
      onClose={onClose}
    >
      <Controller
        control={form.control}
        name="username"
        rules={{ required: "Enter a username." }}
        render={(controller) => (
          <TextField
            {...textFieldProps(controller)}
            label="Username"
            autoComplete="off"
            isRequired
            autoFocus
          />
        )}
      />
      <Controller
        control={form.control}
        name="displayName"
        rules={{ required: "Enter the person's name." }}
        render={(controller) => (
          <TextField
            {...textFieldProps(controller)}
            label="Display name"
            description="Shown in the app and the audit trail."
            autoComplete="off"
            isRequired
          />
        )}
      />
      <RoleChecklist roles={roles} />
      <TemporaryPasswordField />
      <AuthenticationField />
    </UserFormDialog>
  );
}
