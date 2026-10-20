import type { Tone } from "@/components/tones";
import type { AuthorityEvidenceKind } from "@/generated/AuthorityEvidenceKind";
import type { BillingResponsibility } from "@/generated/BillingResponsibility";
import type { ClarificationDeficiency } from "@/generated/ClarificationDeficiency";
import type { ItemKind } from "@/generated/ItemKind";
import type { ItemState } from "@/generated/ItemState";
import type { MedicationSource } from "@/generated/MedicationSource";
import type { OrderFilter } from "@/generated/OrderFilter";
import type { OrderStatus } from "@/generated/OrderStatus";
import type { ReflexAuthorization } from "@/generated/ReflexAuthorization";
import type { SequenceRole } from "@/generated/SequenceRole";
import type { SourceKind } from "@/generated/SourceKind";
import type { Choice } from "@/lib/choices";

export const SOURCE_KINDS: readonly Choice<SourceKind>[] = [
  { id: "paper_requisition", label: "Paper requisition" },
  { id: "fax", label: "Fax" },
];

export const BILLING_RESPONSIBILITIES: readonly Choice<BillingResponsibility>[] = [
  { id: "third_party", label: "Third party (insurance)" },
  { id: "patient_self_pay", label: "Patient (self-pay)" },
  { id: "client", label: "Client (ordering clinic)" },
  { id: "not_supplied", label: "Not supplied" },
];

export const REFLEX_AUTHORIZATIONS: readonly Choice<ReflexAuthorization>[] = [
  { id: "authorized", label: "Authorized" },
  { id: "not_authorized", label: "Not authorized" },
  { id: "not_stated", label: "Not stated" },
];

/** The testing sequence role the requisition states; never inferred from the test (REQ-RES-002). */
export const SEQUENCE_ROLES: readonly Choice<SequenceRole>[] = [
  { id: "first_line", label: "First-line test" },
  { id: "confirmation", label: "Confirmation" },
];

export const AUTHORITY_KINDS: readonly Choice<AuthorityEvidenceKind>[] = [
  { id: "wet_signature", label: "Handwritten signature" },
  { id: "electronic_signature", label: "Electronic signature" },
  { id: "intent_attestation", label: "Documented intent to order" },
  { id: "not_supplied", label: "Not supplied" },
];

export const MEDICATION_SOURCES: readonly Choice<MedicationSource>[] = [
  { id: "ordering_clinic", label: "Ordering clinic" },
  { id: "patient", label: "Patient" },
  { id: "other", label: "Other" },
];

export const TEMPLATE_KINDS: readonly Choice<"none" | "clinic_template">[] = [
  { id: "none", label: "No template" },
  { id: "clinic_template", label: "Clinic template" },
];

export const DEFICIENCIES: readonly Choice<ClarificationDeficiency>[] = [
  { id: "requestor_missing", label: "Requestor missing" },
  { id: "test_selection_missing", label: "Test selection missing" },
  { id: "signature_or_intent_required", label: "Signature or intent to order required" },
  { id: "standing_order_expired", label: "Standing order expired" },
  { id: "standing_order_exhausted", label: "Standing order used up" },
  { id: "standing_order_out_of_scope", label: "Test outside the standing order" },
  { id: "reflex_authority_missing", label: "Reflex authority missing" },
  { id: "other_authority", label: "Other authority question" },
];

export const ITEM_KINDS: readonly Choice<ItemKind>[] = [
  { id: "initial", label: "Initial" },
  { id: "add_on", label: "Add-on" },
  { id: "reflex", label: "Reflex" },
];

export const ITEM_STATES: readonly Choice<ItemState>[] = [
  { id: "requested", label: "Requested" },
  { id: "accepted", label: "Accepted" },
  { id: "declined", label: "Declined" },
  { id: "cancelled", label: "Cancelled" },
];

export const ORDER_FILTERS: readonly Choice<OrderFilter>[] = [
  { id: "all", label: "All orders" },
  { id: "needs_decision", label: "Needs a decision" },
  { id: "on_hold", label: "On hold" },
  { id: "accepted", label: "Accepted" },
];

const ORDER_STATUSES: Record<OrderStatus, { label: string; tone: Tone }> = {
  received: { label: "Received, not yet accepted", tone: "warning" },
  accepted: { label: "Accepted", tone: "neutral" },
  rejected: { label: "Rejected", tone: "neutral" },
};

export function orderStatus(status: OrderStatus): { label: string; tone: Tone } {
  return ORDER_STATUSES[status];
}
