// Laboratory wording for interface contracts and acceptance tests (ticket #4555 step 5a;
// REQ-INT-001). Codes come from the engine and carry no patient data. A passed test is the
// LIS's own synthetic check; the partner's acceptance is the laboratory's to qualify.
import type { Tone } from "@/components/tones";
import type { AcceptanceState } from "@/generated/AcceptanceState";
import type { ContractEdge } from "@/generated/ContractEdge";
import type { ContractSummary } from "@/generated/ContractSummary";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import { INTAKE_REASONS } from "../../intakeLabels";

export const ACCEPTANCE: Record<AcceptanceState, { label: string; tone: Tone }> = {
  passed: { label: "Acceptance tests passed", tone: "success" },
  failed: { label: "Acceptance tests failed", tone: "danger" },
  out_of_date: { label: "Acceptance tests out of date", tone: "warning" },
  not_run: { label: "Acceptance tests not run", tone: "warning" },
};

/** An example of the Message and profile term, for the connection's message type. */
export const PROFILE_EXAMPLES: Record<PartnerMessageType, string> = {
  hl7_orders: "Such as HL7 v2.5.1 OML^O21, LOI Edition 5 based",
  hl7_oru: "Such as HL7 v2.5.1 ORU^R01, LRI Edition 5 based",
  report_pdf: "Such as the issued report PDF, one file per revision",
  billing_json: "Such as Billing JSON bil-handoff-v2",
};

export const EDGES: Record<ContractEdge, string> = {
  lis: "The LIS itself",
  lab_engine: "The laboratory's interface engine",
};

/** What each case checks. */
const CASES: Record<string, string> = {
  mappings_complete: "Code mappings approved, each analyte with a UCUM unit",
  message_header: "Message header: the partner, ORU^R01 and version 2.5.1",
  mapped_codes: "The partner's test and analyte codes and the unit are sent",
  censored_value: "A censored value goes as SN, with no interpretation",
  correction: "A correction goes with status C",
  withdrawal: "A withdrawal goes with status W",
  acknowledgement_mode: "Over MLLP, the partner is asked to answer each message",
  file_name: "The file name pattern makes a valid name",
  mapped_order: "A mapped order code makes an order in Received status",
  replay: "An exact replay makes no second order",
  changed_content: "The same message ID with changed content is held",
  unknown_test: "An unknown test code is refused",
  other_sender: "Another sending facility is refused",
  unknown_patient: "An unknown patient waits or is refused, as set",
};

/** Why a case failed. */
const PROBLEMS: Record<string, string> = {
  no_approved_mappings: "no approved code mappings",
  no_test_mapping: "no test is mapped",
  no_analyte_mapping: "no analyte is mapped",
  analyte_without_unit: "an analyte mapping has no UCUM unit",
  header_mismatch: "the header did not read back as expected",
  codes_not_sent: "the partner's codes or unit did not read back",
  censored_not_sn: "the value was not sent as SN",
  status_mismatch: "the status did not read back",
  acknowledgement_not_requested: "MSH-15 and MSH-16 are not empty",
  file_name_invalid: "the pattern does not make a valid file name",
  ack_mismatch: "it got another acknowledgement than expected",
  unexpected_outcome: "it became something else than expected",
};

export const caseText = (code: string) => CASES[code] ?? code.replaceAll("_", " ");

export const problemText = (code: string) =>
  PROBLEMS[code] ?? INTAKE_REASONS[code]?.toLowerCase() ?? code.replaceAll("_", " ");

/** The Overview line, as in mockup 5: `v3 · acceptance tests passed 2026-10-27 …`. */
export function contractText(
  { version, acceptance, testedAt }: ContractSummary,
  formatTime: (at: string) => string,
): string {
  if (version === null) {
    return "Not approved: no patient data goes or comes until it is approved and tested";
  }
  const state = ACCEPTANCE[acceptance].label.toLowerCase();
  const when = acceptance === "passed" || acceptance === "failed" ? testedAt : null;
  return `v${version} · ${state}${when ? ` ${formatTime(when)}` : ""}`;
}
