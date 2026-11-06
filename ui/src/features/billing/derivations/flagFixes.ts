import type { BillingFlag } from "@/generated/BillingFlag";

/** Where a blocking flag is fixed: the screen, its link text, and what to do there. */
export interface FlagFix {
  path: string;
  /** The link text; `reference` is the engine's label for what the flag is about. */
  place: (reference: string | undefined) => string;
  /** What to do, as the Held message says it before "then derive again". */
  action: string;
}

const segment = encodeURIComponent;
const about = (place: string) => (reference: string | undefined) =>
  reference ? `${place} for ${reference}` : place;

/**
 * The blocking flags whose fix has one clear screen, by flag code; the flag's reference is the
 * record that screen opens. Not linked: `date_of_service_unknown` and `collection_review_pending`
 * (the collection is corrected on its accession, which the flag does not name) and
 * `billing_context_conflict` (resolved on the derivation page itself).
 */
const FIXES: Readonly<Record<string, (refId: string) => FlagFix>> = {
  laboratory_billing_identity_missing: () => ({
    path: "/configuration/laboratory",
    place: () => "Laboratory profile",
    action: "Set Billing NPI and Tax ID in the laboratory profile",
  }),
  // The reference is the specimen; its report workspace prepares and releases the final report.
  report_not_final: (specimenId) => ({
    path: `/reports/specimens/${segment(specimenId)}`,
    place: about("Report"),
    action: "Release the specimen's final report",
  }),
  // The reference is the order.
  billing_context_missing: (orderId) => ({
    path: `/billing/orders/${segment(orderId)}`,
    place: about("Order billing"),
    action: "Record the order's billing selection",
  }),
  billing_context_incomplete: (orderId) => ({
    path: `/billing/orders/${segment(orderId)}`,
    place: about("Order billing"),
    action: "Complete the order's billing selection",
  }),
  billing_review_open: () => ({
    path: "/billing/review",
    place: () => "Billing follow-up",
    action: "Resolve the open financial follow-up",
  }),
  mapping_missing: () => ({
    path: "/billing/class-mappings",
    place: () => "Billing class mappings",
    action: "Give every analyte a confirmed billing class",
  }),
};

/** Where a blocking flag is fixed; `null` for an informational flag or one with no clear screen. */
export function flagFix(flag: BillingFlag): FlagFix | null {
  const fix = flag.blocking ? FIXES[flag.code] : undefined;
  return fix ? fix(flag.refId) : null;
}

/**
 * The Held message: what to do when every blocking flag has the same fix, otherwise a line for
 * several causes.
 */
export function heldMessage(flags: readonly BillingFlag[]): string {
  const blocking = flags.filter((flag) => flag.blocking);
  const count = `${blocking.length} blocking ${blocking.length === 1 ? "flag holds" : "flags hold"} this derivation.`;
  const actions = new Set(blocking.map((flag) => flagFix(flag)?.action ?? null));
  const [action] = actions;
  if (actions.size === 1 && action) return `${count} ${action}, then derive again.`;
  if (actions.size === 1) return `${count} Resolve the cause, then derive again.`;
  return `${count} Fix each blocking flag below, using its Fix in link where there is one, then derive again.`;
}
