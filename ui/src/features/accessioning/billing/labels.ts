import type { BillingReviewElement } from "@/generated/BillingReviewElement";
import type { Choice } from "@/lib/choices";

export const REVIEW_ELEMENTS: readonly Choice<BillingReviewElement>[] = [
  { id: "diagnosis", label: "Supplied diagnosis" },
  { id: "lcd_rationale", label: "Medical necessity documentation" },
  { id: "payer_signature", label: "Payer-required signature" },
  { id: "enrollment_status", label: "Provider enrollment" },
  { id: "other", label: "Other financial evidence" },
];

export const READINESS: readonly Choice<string>[] = [
  { id: "financial_context_missing", label: "Confirm the order's billing selection." },
  { id: "billing_responsibility_missing", label: "Confirm who is responsible for payment." },
  { id: "bill_to_client_missing", label: "Select the bill-to client account." },
  { id: "coverage_missing", label: "Select the coverage confirmed for this order." },
  { id: "member_id_missing", label: "Obtain the missing member identifier from the source." },
  { id: "subscriber_missing", label: "Obtain the missing subscriber details from the source." },
  { id: "billing_address_missing", label: "Obtain the patient billing address from the source." },
  { id: "billing_review_open", label: "Review the open financial follow-up below." },
];
