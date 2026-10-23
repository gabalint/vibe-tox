import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";

/** Account IDs are UUIDs; system components such as `result-import` act under their own name. */
const ACCOUNT_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Who did something, by the account's display name, the signed-in person included. The engine
 * resolves the name; when it sent none for the signed-in person, the session's name is used.
 * Without a name, `unnamed` when the screen sets it (the operations views say "another user"),
 * else "user <ID>" for an account the engine could not name, or the component's own name for
 * a system actor.
 */
export function usePersonText(unnamed?: string): (userId: string, display?: string) => string {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const me = session.data;
  return (userId, display) =>
    display ??
    (userId === me?.userId ? me.displayName : undefined) ??
    unnamed ??
    (ACCOUNT_ID.test(userId) ? `user ${userId}` : userId);
}
