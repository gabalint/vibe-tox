import type { SessionInfo } from "@/generated/SessionInfo";

/** A synthetic signed-in person for auth tests. */
export const TEST_SESSION: SessionInfo = {
  userId: "00000000-0000-7000-8000-000000000001",
  username: "analyst",
  displayName: "Test Analyst",
  roles: ["analyst"],
  capabilities: ["perform_analysis"],
  idleTimeoutSeconds: 900,
  locked: false,
};
