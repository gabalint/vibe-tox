// Catalog preview and approval refusals, with the engine message naming affected records.
import type { CommandError } from "@/generated/CommandError";

/** Generating or saving a Catalog Preview. */
export function previewRefusal(error: CommandError): string | null {
  switch (error.code) {
    case "conflict":
      return "The catalog changed while the report was generated. Generate it again.";
    case "not_found":
      return "This report or its stored file was not found. Generate a new Catalog Preview.";
    case "unavailable":
      return "The PDF could not be saved to your Downloads folder.";
    case "forbidden":
      return "Your roles do not include this work. Ask your laboratory administrator about access.";
    default:
      return null;
  }
}

/** Showing a Catalog Preview; the engine's message explains a missing or damaged file. */
export function viewRefusal(error: CommandError): string | null {
  return error.code === "unavailable" ? null : previewRefusal(error);
}

/**
 * Approve reviewed catalog. A wrong password shows on the password field instead; `null` then, and
 * for codes the engine's message explains alone.
 */
export function reviewRefusal(error: CommandError): string | null {
  switch (error.code) {
    case "forbidden":
      return "Your roles do not include approving configuration. Ask your laboratory administrator about access.";
    case "gate_blocked":
      return "You wrote drafts in this report, and the laboratory's separation-of-duties policy needs a second person to approve them. A configuration approver who did not write them must review them. An administrator can change the policy when authorized by the laboratory.";
    case "validation":
      return error.fieldErrors.length > 0
        ? null
        : "Confirm exactly the blank cutoffs listed. If the list changed, close this dialog and open it again.";
    case "conflict":
      return "The catalog changed after this report was generated, or the report was already reviewed. Generate a new Catalog Preview, check it, and review that one.";
    case "not_found":
      return "This report was not found. Refresh the list of reports.";
    default:
      return null;
  }
}
