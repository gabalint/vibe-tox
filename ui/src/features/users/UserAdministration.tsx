import type { UseQueryResult } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type GridRowActions } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { FindCount, FindField, findRows, NoResults } from "@/components/FindField";
import { QueryContent } from "@/components/QueryContent";
import type { ActionItem } from "@/components/RowActions";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import type { CommandError } from "@/generated/CommandError";
import type { RoleInfo } from "@/generated/RoleInfo";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UserSummary } from "@/generated/UserSummary";
import { type Choice, choiceOf } from "@/lib/choices";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { useFindParam } from "@/lib/findParam";
import type { OpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { useSearchParam } from "@/lib/searchParam";
import { EditRolesDialog } from "./EditRolesDialog";
import { ResetCredentialsDialog } from "./ResetCredentialsDialog";
import { UserPanel } from "./UserPanel";
import { UserStatusDialog } from "./UserStatusDialog";
import { roleNamesOf, userColumns } from "./userColumns";

type Pending = { kind: "roles" | "status" | "reset"; userId: string } | null;

type StatusFilter = "all" | "active" | "disabled";

const STATUS_FILTERS: readonly Choice<StatusFilter>[] = [
  { id: "all", label: "Any status" },
  { id: "active", label: "Active" },
  { id: "disabled", label: "Disabled" },
];

interface UserAdministrationProps {
  users: UseQueryResult<UserSummary[], CommandError>;
  roles: UseQueryResult<RoleInfo[], CommandError>;
  record: OpenRecord;
  /** Fetches the user list again, after a change was refused as stale. */
  onReload: () => void;
}

const getRowId = (user: UserSummary) => user.userId;
const NO_USERS: readonly UserSummary[] = [];
const NO_ROLES: readonly RoleInfo[] = [];

/**
 * The user directory, its side panel and the account actions. `UserSummary` has no
 * `allowedActions` yet, so the offers follow the session as before: account actions need
 * `manage_users`, and nobody is offered changes to their own roles or sign-in. Presentation
 * only: the engine refuses both anyway.
 */
export function UserAdministration({ users, roles, record, onReload }: UserAdministrationProps) {
  const [pending, setPending] = useState<Pending>(null);
  const userRows = users.data ?? NO_USERS;
  const roleRows = roles.data ?? NO_ROLES;
  const roleNames = useMemo(() => roleNamesOf(roleRows), [roleRows]);
  const actionsOf = useUserActions(setPending);
  return (
    <DetailLayout
      // Looked up in the latest list, so a dialog always sends the current revision.
      panel={
        roles.data &&
        openRecordPanel(record, userRows, getRowId, (open) => (
          <UserPanel
            user={open}
            roleNames={roleNames}
            actions={actionsOf.of(open)}
            own={actionsOf.isOwn(open)}
            record={record}
          />
        ))
      }
    >
      <UserList
        users={users}
        roles={roles}
        record={record}
        rowActions={{
          label: (user) => user.username,
          hasItems: (user) => actionsOf.of(user).length > 0,
          items: actionsOf.of,
        }}
      />
      <UserDialogs
        pending={pending}
        users={userRows}
        roles={roleRows}
        onClose={() => setPending(null)}
        onReload={onReload}
      />
    </DetailLayout>
  );
}

interface UserListProps {
  users: UseQueryResult<UserSummary[], CommandError>;
  roles: UseQueryResult<RoleInfo[], CommandError>;
  record: OpenRecord;
  rowActions: GridRowActions<UserSummary>;
}

/** Find, the status filter and the one grid, whose rows open the user's panel. */
function UserList({ users, roles, record, rowActions }: UserListProps) {
  const [query, setQuery] = useFindParam();
  const [status, setStatus] = useStatusFilter();
  const userRows = users.data ?? NO_USERS;
  const roleRows = roles.data ?? NO_ROLES;
  const columns = useMemo(() => userColumns(roleRows), [roleRows]);
  const shown = useMemo(
    () =>
      findRows(
        userRows.filter((user) => status === "all" || user.status === status),
        query,
        (user) => [user.displayName, user.username],
      ),
    [userRows, status, query],
  );
  return (
    <>
      <Toolbar
        variant="collection"
        count={
          users.data &&
          !users.isError && (
            <FindCount shown={shown.length} total={userRows.length} noun="users" one="user" />
          )
        }
      >
        <FindField
          label="Find users"
          placeholder="Name or user name"
          value={query}
          onChange={setQuery}
        />
        <Select
          layout="inline"
          label="Status"
          name="status"
          options={STATUS_FILTERS}
          value={status}
          onChange={(next) => setStatus(choiceOf(STATUS_FILTERS, next) ?? "all")}
        />
        {status !== "all" && (
          <Button variant="quiet" onPress={() => setStatus("all")}>
            Clear filters
          </Button>
        )}
      </Toolbar>
      <QueryContent
        query={users}
        what="users"
        loadingRows={8}
        empty={{
          when: (rows) => rows.length === 0,
          title: "No users yet",
          description: "Create the first user to give them access.",
        }}
      >
        {() => (
          <QueryContent query={roles} what="roles" loadingRows={8}>
            {() =>
              shown.length === 0 ? (
                <NoResults
                  noun="users"
                  query={query}
                  onClear={() => (query.trim() ? setQuery("") : setStatus("all"))}
                />
              ) : (
                <DataGrid
                  label="Users"
                  columns={columns}
                  rows={shown}
                  getRowId={getRowId}
                  openRowId={record.openId}
                  onOrderChange={record.followOrder}
                  onRowAction={(user, order) => record.open(user.userId, order)}
                  rowActions={rowActions}
                  fill
                />
              )
            }
          </QueryContent>
        )}
      </QueryContent>
    </>
  );
}

interface UserDialogsProps {
  pending: Pending;
  users: readonly UserSummary[];
  roles: readonly RoleInfo[];
  onClose: () => void;
  onReload: () => void;
}

/** The one dialog asked for, looking the user up in the latest list so it sends the current revision. */
function UserDialogs({ pending, users, roles, onClose, onReload }: UserDialogsProps) {
  const target = users.find((user) => user.userId === pending?.userId);
  if (!pending || !target) return null;
  const reload = () => {
    onReload();
    onClose();
  };
  if (pending.kind === "roles") {
    return <EditRolesDialog user={target} roles={roles} onClose={onClose} onReload={reload} />;
  }
  if (pending.kind === "status") {
    return <UserStatusDialog user={target} onClose={onClose} onReload={reload} />;
  }
  return <ResetCredentialsDialog user={target} onClose={onClose} onReload={reload} />;
}

/** The status filter, kept in the address with the find text and the open user. */
function useStatusFilter(): [StatusFilter, (next: StatusFilter) => void] {
  const [status, setStatus] = useSearchParam("status");
  return [
    choiceOf(STATUS_FILTERS, status) ?? "all",
    (next) => setStatus(next === "all" ? null : next),
  ];
}

/**
 * The account actions offered on one user, as before this screen had a panel: Edit roles
 * to anyone but the person themselves (a quality manager assigns clinical roles), and
 * Disable or Enable and Reset sign-in to `manage_users`, Reset never on one's own account.
 *
 * INTERIM, until `UserSummary` carries `allowedActions`: the engine also refuses Reset
 * sign-in for a user with a clinical role unless the administrator holds `manage_quality`
 * (`lis-auth` duties), and this screen cannot know that without copying the rule. The offer
 * is shown anyway and the engine's refusal is displayed. Engine request: `allowedActions`
 * on `UserSummary` (edit roles, enable, disable, reset sign-in), computed for the caller.
 */
function useUserActions(setPending: (pending: Pending) => void) {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION).data;
  const managesAccounts = session?.capabilities.includes("manage_users") ?? false;
  const isOwn = (user: UserSummary) => user.userId === session?.userId;
  const of = (user: UserSummary): ActionItem[] => {
    const own = isOwn(user);
    const ask = (kind: "roles" | "status" | "reset") => () =>
      setPending({ kind, userId: user.userId });
    const items: ActionItem[] = [];
    if (!own) items.push({ id: "roles", label: "Edit roles", onAction: ask("roles") });
    if (managesAccounts) {
      items.push(
        user.status === "disabled"
          ? { id: "status", label: "Enable user", onAction: ask("status") }
          : { id: "status", label: "Disable user", variant: "danger", onAction: ask("status") },
      );
      if (!own && user.authenticationMethod !== "security_key")
        items.push({ id: "reset", label: "Reset sign-in", onAction: ask("reset") });
    }
    return items;
  };
  return { of, isOwn };
}
