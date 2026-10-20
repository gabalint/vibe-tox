import type { SeparationPolicy } from "@/generated/SeparationPolicy";

/**
 * The duties of the separation-of-duties policy (ADR 0015), in laboratory terms: what one
 * person does both steps of when the laboratory allows it.
 */
export const DUTIES: readonly { id: keyof SeparationPolicy; label: string; details: string }[] = [
  {
    id: "selfApproveConfiguration",
    label: "Approve configuration they wrote",
    details:
      "Method, test and panel, validity policy and billing class mapping versions, All Data Reviewed, and result sources.",
  },
  {
    id: "selfReleaseReports",
    label: "Release a report whose results they reviewed",
    details: "The technical reviewer of reported results releases the report.",
  },
  {
    id: "selfVerifyDestinations",
    label: "Verify a delivery destination they created",
    details: "The person who added a destination confirms that it reaches the client.",
  },
];
