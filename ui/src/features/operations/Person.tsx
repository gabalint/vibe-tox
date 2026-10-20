import { usePersonText } from "@/lib/person";

/**
 * Who did something, from the principal ID the engine sends: "you" for the signed-in person,
 * otherwise "another user". The ID itself says nothing to a reader, so it never shows.
 * Engine request: a display name beside each principal ID in the operations views, as
 * delivery destinations carry `createdBy.displayName`.
 */
export function Person({ id }: { id: string }) {
  return usePersonText("another user")(id);
}
