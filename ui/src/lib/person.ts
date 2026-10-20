import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";

/**
 * Who did something, as a reader says it: "you" for the signed-in person, otherwise the
 * display name the engine resolved. Without one, `unnamed` when the screen sets it (the
 * operations views say "another user", since the engine sends them no names and an ID says
 * nothing to a reader), or else "user <ID>".
 */
export function usePersonText(unnamed?: string): (userId: string, display?: string) => string {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const me = session.data?.userId;
  return (userId, display) => {
    if (userId === me) return "you";
    return display ?? unnamed ?? `user ${userId}`;
  };
}
