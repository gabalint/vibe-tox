// Laboratory wording for the Interfaces codes the engine returns. Tones follow meaning: a
// written file whose use the folder cannot confirm is neutral, never success (REQ-RPT-011).
import type { Tone } from "@/components/tones";
import type { ConnectionChange } from "@/generated/ConnectionChange";
import type { ConnectionStatus } from "@/generated/ConnectionStatus";
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
  billing_json: "Billing JSON",
};

/** What makes each message type, shown as the connection's "Send when". */
export const SEND_WHEN: Record<PartnerMessageType, string> = {
  billing_json: "Billing handoff created",
};

export const PROTOCOLS = { folder: "Folder" } as const;

export const STATUS: Record<ConnectionStatus, Labelled> = {
  active: { label: "Active", tone: "success" },
  stopped: { label: "Stopped", tone: "neutral" },
  error: { label: "Error", tone: "danger" },
};

export const RULE_FIELDS: Record<RuleField, string> = {
  client: "Client",
  practitioner: "Practitioner (NPI)",
  location: "Location",
  test: "Test",
};

export const RULE_CONDITIONS: Record<RuleCondition, string> = {
  equals: "equals",
  one_of: "is one of",
  starts_with: "starts with",
};

export const ruleText = (rule: RoutingRule) =>
  `${RULE_FIELDS[rule.field]} ${RULE_CONDITIONS[rule.condition]} ${rule.value}`;

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

/** The record a row is about, such as "Billing handoff 12". */
export const recordText = (row: { sourceNumber: number | null }) =>
  row.sourceNumber === null ? "Test message" : `Billing handoff ${row.sourceNumber}`;

/** One line on why the row stands where it does. */
export function detailText(row: PartnerMessageRow, formatTime: (at: string) => string): string {
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
