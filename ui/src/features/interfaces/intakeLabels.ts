// Laboratory wording for received order messages: what each became and why (ticket #4555
// step 3c). Codes come from the engine and carry no patient data.
import type { Tone } from "@/components/tones";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { ReceivedState } from "@/generated/ReceivedState";

/** Why a received message waits, is held or was refused. */
export const INTAKE_REASONS: Record<string, string> = {
  patient_unknown: "No registered patient has the message's identifier",
  patient_identifier_conflict: "The identifiers belong to more than one patient",
  patient_details_differ: "The patient's name or birth date differs from the registered patient",
  changed_content: "Same message ID with changed content",
  not_accepted_sender: "Not from the accepted sending facility (MSH-4)",
  unknown_test: "Unknown test code: no approved code mapping",
  duplicate_test: "The same test is ordered twice",
  unknown_provider: "Unknown ordering provider NPI",
  no_provider: "No ordering provider",
  unknown_location: "Unknown client location (PV1-3)",
  unknown_client: "The connection's client is not active",
  diagnosis_system_missing: "A diagnosis has no coding system",
  order_refused: "The order rules refused the order",
  not_text: "Not UTF-8 text",
  not_hl7: "Not an HL7 message",
  unsupported_message: "Not an OML^O21 or ORM^O01 order message",
  no_control_id: "No message control ID (MSH-10)",
  no_patient: "No patient identifier (PID-3)",
  no_order: "No order (ORC and OBR)",
  no_test: "An OBR without a test code",
  no_placer_order: "No placer order number",
  unsupported_control: "Only new orders (ORC-1 NW) are read",
};

const OUTCOMES: Record<ReceivedState, { label: string; tone: Tone }> = {
  order_created: { label: "Order created", tone: "success" },
  waiting_patient: { label: "Waiting for patient", tone: "warning" },
  held: { label: "Held for review", tone: "warning" },
  duplicate: { label: "Duplicate – ignored", tone: "neutral" },
  rejected: { label: "Rejected", tone: "danger" },
};

/** What a received message became, as the Application column shows it. */
export function receivedOutcome(state: ReceivedState, reason: string | null) {
  const outcome = OUTCOMES[state];
  return state === "rejected" && reason === "unknown_test"
    ? { ...outcome, label: "Rejected – unknown test" }
    : outcome;
}

/** Why a received message stands where it does; `null` for a new order. An order made after
 * its patient was registered keeps only that it waited. */
function receivedReason(row: PartnerMessageRow): string | null {
  if (row.received === "duplicate") return "Exact replay; no second order";
  if (row.errorCode === null) return null;
  return row.received === "order_created"
    ? "Waited until the patient was registered"
    : (INTAKE_REASONS[row.errorCode] ?? row.errorCode);
}

/** Why, and where the file went, or over MLLP the acknowledgement the partner got. */
export function receivedDetail(row: PartnerMessageRow): string {
  const answered =
    row.peer !== null && row.ackCode !== null ? `ACK ${row.ackCode} returned to ${row.peer}` : null;
  const moved = row.writtenPath === null ? null : `Moved to ${row.writtenPath}`;
  return [receivedReason(row), answered ?? moved].filter(Boolean).join(" · ");
}
