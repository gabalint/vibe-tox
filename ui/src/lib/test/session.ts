import type { SessionInfo } from "@/generated/SessionInfo";

/** A signed-in session holding `capabilities`. */
export function session(capabilities: readonly string[]): SessionInfo {
  return {
    userId: "00000000-0000-7000-8000-000000000003",
    username: "reviewer",
    displayName: "Robin Reviewer",
    roles: ["administrator"],
    capabilities: [...capabilities],
    idleTimeoutSeconds: 900,
    locked: false,
  };
}
