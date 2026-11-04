// Laboratory wording for the Interfaces codes the engine returns. Tones follow meaning: a
// written file whose use the folder cannot confirm is neutral, never success (REQ-RPT-011).
import type { Tone } from "@/components/tones";
import type { ConnectionChange } from "@/generated/ConnectionChange";
import type { ConnectionStatus } from "@/generated/ConnectionStatus";
import type { MessageSource } from "@/generated/MessageSource";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import type { RoutingRule } from "@/generated/RoutingRule";
import type { RuleCondition } from "@/generated/RuleCondition";
import type { RuleField } from "@/generated/RuleField";
import type { Schedule } from "@/generated/Schedule";
import type { UnroutedResolution } from "@/generated/UnroutedResolution";
import type { WaitReason } from "@/generated/WaitReason";

export interface Labelled {
  label: string;
  tone: Tone;
}

export const MESSAGE_TYPES: Record<PartnerMessageType, string> = {
  hl7_oru: "HL7 Results (ORU^R01)",
  report_pdf: "Report PDF",
  billing_json: "Billing JSON",
};

/** The Message choices of the Add connection dialog, as in the mockup. */
export const MESSAGE_OPTIONS: Record<PartnerMessageType, string> = {
  ...MESSAGE_TYPES,
  hl7_oru: "HL7 Results (ORU^R01, v2.5.1)",
};

/** What makes each message type, shown as the connection's "Send when". */
export const SEND_WHEN: Record<PartnerMessageType, string> = {
  hl7_oru: "Results released",
  report_pdf: "Report released",
  billing_json: "Billing handoff created",
};

/** What a connection of each type writes, its message, and when it is made. */
const WRITES: Record<PartnerMessageType, { file: string; message: string; when: string }> = {
  hl7_oru: {
    file: "an HL7 ORU result message with the partner's codes",
    message: "HL7 ORU^R01, v2.5.1",
    when: "results are released",
  },
  report_pdf: {
    file: "the issued report PDF",
    message: "Report PDF (the issued PDF, as released)",
    when: "a report revision is released",
  },
  billing_json: {
    file: "the billing handoff file",
    message: "Billing JSON (bil-handoff-v2)",
    when: "a billing handoff is created",
  },
};

/** The connection page's one-line description, as in the mockup. */
export const connectionDescription = (type: PartnerMessageType, partner: string) =>
  `Writes ${WRITES[type].file} to ${partner}'s folder when ${WRITES[type].when}.`;

/** The message with its format, such as "Billing JSON (bil-handoff-v2)". */
export const messageText = (type: PartnerMessageType) => WRITES[type].message;

/** What goes to a connection without rules. */
export const noRulesText = (type: PartnerMessageType) =>
  `None: every ${type === "billing_json" ? "billing handoff" : "released report"} goes to this connection`;

/** Each message type's file name rule, shown under the File name field. */
export const FILE_NAMES: Record<PartnerMessageType, { pattern: string; hint: string }> = {
  hl7_oru: {
    pattern: "{accession}_r{revision}_{date}.hl7",
    hint: "{accession} (the report number) and {revision} are required; {date} and {time} are optional; ends in .hl7.",
  },
  report_pdf: {
    pattern: "{accession}_r{revision}_{date}.pdf",
    hint: "{accession} (the report number) and {revision} are required; {date} and {time} are optional; ends in .pdf.",
  },
  billing_json: {
    pattern: "billing-{handoff}_{date}.json",
    hint: "{handoff} is required; {date} and {time} are optional; ends in .json.",
  },
};

export const PROTOCOLS = { folder: "Folder" } as const;

export const STATUS: Record<ConnectionStatus, Labelled> = {
  active: { label: "Active", tone: "success" },
  stopped: { label: "Stopped", tone: "neutral" },
  error: { label: "Error", tone: "danger" },
};

const RULE_FIELDS: Record<RuleField, string> = {
  client: "Client",
  practitioner: "Practitioner (NPI)",
  location: "Location",
  test: "Test",
};

/** An HL7 connection names the same LIS values by the HL7 fields that carry them. */
const HL7_RULE_FIELDS: Record<RuleField, string> = {
  client: "ORC-21 Ordering facility",
  practitioner: "OBR-16 Ordering provider (NPI)",
  location: "PV1-3 Location",
  test: "OBR-4 Test (LIS code)",
};

/** The rule fields' names for a connection's message type. */
export const ruleFields = (type: PartnerMessageType | "") =>
  type === "hl7_oru" ? HL7_RULE_FIELDS : RULE_FIELDS;

export const RULE_CONDITIONS: Record<RuleCondition, string> = {
  equals: "equals",
  one_of: "is one of",
  starts_with: "starts with",
};

export const ruleText = (rule: RoutingRule, type: PartnerMessageType) =>
  `${ruleFields(type)[rule.field]} ${RULE_CONDITIONS[rule.condition]} ${rule.value}`;

export function scheduleText(schedule: Schedule): string {
  switch (schedule.kind) {
    case "immediately":
      return "Immediately";
    case "every_minutes":
      return `Every ${schedule.minutes} minutes`;
    case "daily_at":
      return `Daily at ${schedule.time}`;
  }
}

export const CHANGES: Record<ConnectionChange, string> = {
  created: "Added",
  updated: "Changed",
  started: "Started",
  stopped: "Stopped",
  verified: "Destination check recorded",
  mappings_approved: "Code mappings approved",
};

/** The engine's PHI-free write problems. */
const PROBLEMS: Record<string, string> = {
  folder_missing: "Folder not found or not reachable",
  access_denied: "This computer may not write there",
  disk_full: "The disk is full",
  name_taken: "No free file name",
  write_failed: "The file could not be written",
  file_unavailable: "The stored message file is missing",
};

export const problemText = (code: string) => PROBLEMS[code] ?? code.replaceAll("_", " ");

const WAITING: Record<WaitReason, string> = {
  stopped: "Connection stopped",
  unverified: "Destination check not recorded",
  scheduled: "Waiting for the connection's schedule",
  queued: "Written at the next pass",
  impact_review: "Paused while the report's impact review is open",
  not_mapped: "Held: a test or analyte has no approved code mapping",
};

const RESOLUTIONS: Record<UnroutedResolution, string> = {
  sent_once: "Sent once to a connection",
  routed_again: "Routed again by the rules",
  delivered_another_way: "Delivery recorded another way",
  dismissed: "Dismissed",
};

/** Did it leave: what the folder can prove. */
export function transportOf(row: PartnerMessageRow): Labelled {
  switch (row.transport) {
    case "written":
      return { label: "Written", tone: "neutral" };
    case "failed":
      return { label: "Failed", tone: "danger" };
    case "not_sent":
      return { label: "Not sent", tone: row.unrouted ? "warning" : "hold" };
  }
}

/** Did the partner accept it: a folder never says. */
export const applicationOf = (row: PartnerMessageRow): Labelled | null =>
  row.outcomeUnknown ? { label: "Outcome unknown", tone: "warning" } : null;

/** A message's routing values by their display labels, such as client names. */
export const valuesText = (values: readonly { label: string }[]) =>
  values.map((value) => value.label).join(", ") || "None";

/** The record a row is about, such as "Billing handoff 12" or "A26000005-U r1". */
export function recordText(row: {
  source: MessageSource;
  sourceNumber: number | null;
  sourceLabel: string | null;
}): string {
  switch (row.source) {
    case "billing_handoff":
      return `Billing handoff ${row.sourceNumber}`;
    case "report_revision":
      return `${row.sourceLabel} r${row.sourceNumber}`;
    case "report_withdrawal":
      return `${row.sourceLabel} withdrawal notice`;
    case "test":
      return "Test message";
  }
}

/** A report's later revision or withdrawal, sent first to a connection that had the report. */
function followUpText(row: PartnerMessageRow): string | null {
  if (row.reason !== "follow_up") return null;
  return row.source === "report_withdrawal"
    ? "Withdrawal notice, sent first"
    : `Replaces r${(row.sourceNumber ?? 1) - 1}, sent first`;
}

function stateText(row: PartnerMessageRow, formatTime: (at: string) => string): string {
  if (row.superseded) return "Replaced by a newer revision; never written";
  if (row.unrouted) {
    return row.resolution === null
      ? "No connection rule matched; waiting in Unrouted messages"
      : RESOLUTIONS[row.resolution];
  }
  if (row.writtenPath !== null) return `Written as ${row.writtenPath}`;
  const problem = row.errorCode === null ? "" : problemText(row.errorCode);
  if (row.givenUp) {
    return `${problem}; not written after ${row.attempts} attempt${row.attempts === 1 ? "" : "s"}`;
  }
  if (row.attempts > 0 && row.nextAttemptAt !== null) {
    return `${problem}; attempt ${row.attempts + 1} of 5 at ${formatTime(row.nextAttemptAt)}`;
  }
  return row.waiting === null ? "" : WAITING[row.waiting];
}

/** One line on why the row stands where it does. */
export function detailText(row: PartnerMessageRow, formatTime: (at: string) => string): string {
  return [followUpText(row), stateText(row, formatTime)].filter(Boolean).join(" · ");
}
