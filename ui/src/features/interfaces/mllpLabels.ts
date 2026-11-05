// Laboratory wording for MLLP connections (ticket #4555 step 4): the partner's address, its
// acknowledgements and the transport's problems. Codes come from the engine and carry no
// patient data; an acknowledgement is the partner's answer, never proof a clinician saw it.
import type { Tone } from "@/components/tones";
import type { AckCode } from "@/generated/AckCode";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { MllpPartner } from "@/generated/MllpPartner";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";

/** An address and port as the LIS connects to them; an IPv6 address goes in brackets. */
export const socketText = ({ address, port }: Pick<MllpPartner, "address" | "port">) =>
  `${address.includes(":") ? `[${address}]` : address}:${port}`;

/** Why a message sent over MLLP got no acknowledgement, or a test failed. */
export const MLLP_PROBLEMS: Record<string, string> = {
  not_lab_network: "The address is not on the lab network",
  partner_unreachable: "The partner did not answer; check the address, port and network",
  connection_lost: "The connection closed before the partner's acknowledgement",
  no_ack: "No acknowledgement in time",
  ack_unreadable: "The partner's answer was not an HL7 acknowledgement (AA, AE or AR)",
  ack_other_message: "The partner acknowledged another message",
  no_control_id: "The message has no control ID (MSH-10)",
};

const ACKS: Record<AckCode, { label: string; tone: Tone; answer: string }> = {
  AA: { label: "Accepted (AA)", tone: "success", answer: "accepted it (AA)" },
  AE: { label: "Rejected (AE)", tone: "danger", answer: "found an error (AE)" },
  AR: { label: "Rejected (AR)", tone: "danger", answer: "rejected it (AR)" },
};

/** What the partner answered, or that its answer is still missing; `null` without MLLP. */
export function ackOutcome(row: PartnerMessageRow): { label: string; tone: Tone } | null {
  if (row.ackCode !== null) return ACKS[row.ackCode];
  return row.awaitingAck ? { label: "Awaiting application ACK", tone: "warning" } : null;
}

/** Where it was sent and, when the partner did not accept it, the partner's reason. */
export function sentText(row: PartnerMessageRow): string {
  const reason = row.ackCode !== "AA" && row.ackText ? ` · Partner: ${row.ackText}` : "";
  return `Sent to ${row.writtenPath}${reason}`;
}

/** The partner's answer to a synthetic test message. */
export function testAnswerText(result: FolderTestResult): string {
  if (!result.ackCode) return "";
  const reason = result.ackText ? `: ${result.ackText}` : ".";
  return `Synthetic test message sent; the partner ${ACKS[result.ackCode].answer}${reason}`;
}

/** The line after Test connection in the dialog. */
export function mllpTestText({ problem }: FolderTestResult, partner: MllpPartner): string {
  if (problem === null) return `✓ ${socketText(partner)} takes a connection; nothing was sent.`;
  return `✗ Not reachable: ${MLLP_PROBLEMS[problem] ?? problem.replaceAll("_", " ")}.`;
}
