// What each starter data refusal means and what to do next, in laboratory terms. The engine's
// own message follows, naming the records concerned.
import type { CommandError } from "@/generated/CommandError";

const NO_ROLE = "Your roles do not include this work. A laboratory administrator can change roles.";

/** The Standard Catalog load. */
export function loadRefusal(error: CommandError): string | null {
  switch (error.code) {
    case "conflict":
      return "The Standard Catalog or your catalog changed since this list was shown, or an item cannot be loaded. Reload the list and choose again.";
    case "validation":
      return "Every method, test and panel needs at least one analyte, and the disclaimer must be acknowledged.";
    case "forbidden":
      return NO_ROLE;
    default:
      return null;
  }
}
