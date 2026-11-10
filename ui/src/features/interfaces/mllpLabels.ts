// Laboratory wording for MLLP connections (ticket #4555 step 4): the partner's address, its
// acknowledgements, the LIS's listener and the transport's problems. Codes come from the
// engine and carry no patient data; an acknowledgement is the partner's answer, never proof a
// clinician saw it.
import type { Tone } from "@/components/tones";
import type { AckCode } from "@/generated/AckCode";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { ListenerView } from "@/generated/ListenerView";
import type { MllpListener } from "@/generated/MllpListener";
import type { MllpPartner } from "@/generated/MllpPartner";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";

/** An address and port as the LIS connects to them; an IPv6 address goes in brackets. */
export const socketText = ({ address, port }: Pick<MllpPartner, "address" | "port">) =>
  `${address.includes(":") ? `[${address}]` : address}:${port}`;

/**
 * Whether the connection waits for a partner's application ACK: only when the LIS sends over
 * MLLP. A folder or SFTP server never answers, and a listener sends the ACKs itself (#4704).
 */
export const awaitsAck = (connection: PartnerConnection) =>
  connection.settings.protocol === "mllp_connect";

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

/** Why the LIS cannot listen on an address and port. */
export const LISTENER_PROBLEMS: Record<string, string> = {
  port_in_use: "The port is in use by another program or connection",
  address_not_on_this_computer: "The address is not one of this computer's addresses",
  port_not_allowed: "This computer does not let the LIS listen on this port; use one above 1023",
  listener_failed: "The LIS could not listen there",
};

const listenerProblem = (code: string) =>
  LISTENER_PROBLEMS[code] ?? MLLP_PROBLEMS[code] ?? code.replaceAll("_", " ");

/** The line after Test listener in the dialog, as in mockup 2. */
export function listenerTestText({ problem }: FolderTestResult, listener: MllpListener): string {
  if (problem === null) return `✓ Port ${listener.port} free on ${listener.address}.`;
  return `✗ Cannot listen: ${listenerProblem(problem)}.`;
}

/** Whether the listener listens now, as the connection page shows it. */
export function listeningText(
  listener: ListenerView | null,
  formatTime: (at: string) => string,
): string {
  if (listener === null) {
    return "Not listening: the connection is stopped, not checked, or its acceptance tests have not passed";
  }
  if (listener.listening) return `Listening since ${formatTime(listener.since)}`;
  const why = listener.problem === null ? "" : `: ${listenerProblem(listener.problem)}`;
  return `Not listening${why} (since ${formatTime(listener.since)}); tried again every 5 seconds`;
}
