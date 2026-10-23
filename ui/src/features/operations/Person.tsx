import { usePersonText } from "@/lib/person";

/**
 * Who did something, from the principal ID and the display name the engine resolved, the
 * signed-in person included. Without a name (a system principal or a removed account),
 * "another user"; the ID itself says nothing to a reader, so it never shows.
 */
export function Person({ id, display }: { id: string; display?: string | undefined }) {
  return usePersonText("another user")(id, display);
}
