import type { RoleInfo } from "@/generated/RoleInfo";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UserSummary } from "@/generated/UserSummary";

/** Synthetic roles and users for the user administration tests. */
export const TEST_ROLES: RoleInfo[] = [
  {
    code: "administrator",
    label: "Administrator",
    description: "Manages users and configuration.",
    capabilities: ["manage_users", "manage_configuration"],
    assignable: true,
  },
  {
    code: "analyst",
    label: "Analyst",
    description: "Runs analyses on the instruments.",
    capabilities: ["perform_analysis"],
    assignable: true,
  },
  {
    code: "auditor",
    label: "Auditor",
    description: "Reviews the audit trail.",
    capabilities: ["view_audit"],
    assignable: true,
  },
  {
    code: "releaser",
    label: "Authorized releaser",
    description: "Releases and corrects reports.",
    capabilities: ["release_reports"],
    assignable: false,
  },
];

export const ADMIN: UserSummary = {
  userId: "00000000-0000-7000-8000-000000000101",
  username: "admin",
  displayName: "Test Administrator",
  status: "active",
  roles: ["administrator"],
  authenticationMethod: "password_totp",
  mfaEnrolled: true,
  lockedOut: false,
  revision: 3,
};

export const ANALYST: UserSummary = {
  userId: "00000000-0000-7000-8000-000000000102",
  username: "analyst",
  displayName: "Test Analyst",
  status: "active",
  roles: ["analyst"],
  authenticationMethod: "password_totp",
  mfaEnrolled: false,
  lockedOut: true,
  revision: 1,
};

export const FORMER: UserSummary = {
  userId: "00000000-0000-7000-8000-000000000103",
  username: "former",
  displayName: "Test Former Analyst",
  status: "disabled",
  roles: [],
  authenticationMethod: "password_totp",
  mfaEnrolled: true,
  lockedOut: false,
  revision: 5,
};

/** The signed-in administrator, as `current_session` answers. */
export const ADMIN_SESSION: SessionInfo = {
  userId: ADMIN.userId,
  username: ADMIN.username,
  displayName: ADMIN.displayName,
  roles: ["administrator"],
  capabilities: ["manage_users", "manage_configuration"],
  idleTimeoutSeconds: 900,
  locked: false,
};
