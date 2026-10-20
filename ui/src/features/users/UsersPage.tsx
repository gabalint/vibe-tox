import { useState } from "react";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import type { RoleInfo } from "@/generated/RoleInfo";
import type { UserSummary } from "@/generated/UserSummary";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { CreateUserDialog } from "./CreateUserDialog";
import { LIST_USERS } from "./contract";
import { OnePersonAction } from "./OnePersonAction";
import { UserAdministration } from "./UserAdministration";

/**
 * User administration (REQ-IDN-004), for people with `manage_users`, and for the quality
 * manager (`manage_quality`), who assigns clinical roles. The engine checks every command and
 * records each change in the audit trail. A row opens the account in a side panel.
 */
export function UsersPage() {
  const roles = useCommandQuery<RoleInfo[]>("list_roles");
  const users = useCommandQuery<UserSummary[]>(LIST_USERS);
  const managesAccounts = useHasCapability("manage_users");
  const record = useOpenRecord();
  const [creating, setCreating] = useState(false);
  return (
    <Page layout="collection">
      <PageHeader
        title="Users"
        actions={
          <>
            {managesAccounts && (
              <Button variant="primary" onPress={() => setCreating(true)}>
                Create user
              </Button>
            )}
            <OnePersonAction users={users.data} />
          </>
        }
      />
      <UserAdministration
        users={users}
        roles={roles}
        record={record}
        onReload={() => void users.refetch()}
      />
      {creating && roles.data && (
        <CreateUserDialog
          roles={roles.data}
          onCreated={(user) => record.open(user.userId)}
          onClose={() => setCreating(false)}
        />
      )}
    </Page>
  );
}
