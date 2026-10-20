import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import type { RoleInfo } from "@/generated/RoleInfo";
import type { UserSummary } from "@/generated/UserSummary";

/** A user's roles as the engine's role labels, such as "Analyst, Auditor". */
export function roleNamesOf(roles: readonly RoleInfo[]): (user: UserSummary) => string {
  const labels = new Map<string, string>(roles.map((role) => [role.code, role.label]));
  return (user) => user.roles.map((code) => labels.get(code) ?? code).join(", ") || "No roles";
}

export const statusLabel = (user: UserSummary) =>
  user.status === "active" ? "Active" : "Disabled";

export const authenticationLabel = (user: UserSummary) =>
  user.authenticationMethod === "security_key" ? "Security key" : "Password + authenticator app";

export function Authenticator({ user }: { user: UserSummary }) {
  return user.mfaEnrolled ? (
    <StatusBadge tone="neutral">Set up</StatusBadge>
  ) : (
    <StatusBadge tone="warning">Not set up</StatusBadge>
  );
}

export function Lockout({ user }: { user: UserSummary }) {
  return user.lockedOut ? <StatusBadge tone="warning">Locked out</StatusBadge> : "None";
}

/** The user grid's columns; role codes show as the engine's role labels. */
export function userColumns(roles: readonly RoleInfo[]): DataGridColumn<UserSummary>[] {
  const roleNames = roleNamesOf(roles);
  return [
    {
      id: "username",
      header: "Username",
      identifier: true,
      cell: (user) => user.username,
      sortValue: (user) => user.username,
      width: 160,
    },
    {
      id: "displayName",
      header: "Display name",
      cell: (user) => user.displayName,
      sortValue: (user) => user.displayName,
    },
    { id: "roles", header: "Roles", cell: roleNames, sortValue: roleNames },
    {
      id: "authentication",
      header: "Sign-in method",
      cell: authenticationLabel,
      sortValue: authenticationLabel,
    },
    {
      id: "mfa",
      header: "Authenticator",
      cell: (user) => <Authenticator user={user} />,
      sortValue: (user) => (user.mfaEnrolled ? 1 : 0),
      width: 150,
    },
    {
      id: "status",
      header: "Status",
      cell: (user) => <StatusBadge tone="neutral">{statusLabel(user)}</StatusBadge>,
      sortValue: (user) => user.status,
      width: 130,
    },
    {
      id: "lockout",
      header: "Sign-in lockout",
      cell: (user) => <Lockout user={user} />,
      sortValue: (user) => (user.lockedOut ? 1 : 0),
      width: 150,
    },
  ];
}
