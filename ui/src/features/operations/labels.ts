import type { Tone } from "@/components/tones";
import type { AnchorSummary } from "@/generated/AnchorSummary";
import type { DisposalMethod } from "@/generated/DisposalMethod";
import type { HealthState } from "@/generated/HealthState";
import type { TargetKind } from "@/generated/TargetKind";
import type { TargetPurpose } from "@/generated/TargetPurpose";
import type { Choice } from "@/lib/choices";

// Labels for operations engine codes (M4 design brief §2.3, §3.6, §6.1). An unknown code shows
// as received; nothing here decides a state.

export interface Status {
  label: string;
  tone: Tone;
}

function statusOf(statuses: Record<string, Status>, code: string): Status {
  return statuses[code] ?? { label: code, tone: "neutral" };
}

const HEALTH_STATES: Record<HealthState, Status> = {
  ok: { label: "OK", tone: "neutral" },
  warning: { label: "Warning", tone: "warning" },
  alert: { label: "Alert", tone: "danger" },
  // The engine counts unknown as a warning, so it is never shown as fine.
  unknown: { label: "Unknown", tone: "warning" },
  attested: { label: "Attested", tone: "neutral" },
};

export function healthState(state: string): Status {
  return statusOf(HEALTH_STATES, state);
}

/**
 * Backup runs, restore tests and exit exports share these job states. A routine verified copy
 * is finished, so it is plain text; `passed` is the outcome of a check the person ran.
 */
const JOB_STATES: Record<string, Status> = {
  requested: { label: "Requested", tone: "info" },
  running: { label: "Running", tone: "info" },
  succeeded: { label: "Verified", tone: "neutral" },
  passed: { label: "Passed", tone: "success" },
  failed: { label: "Failed", tone: "danger" },
  interrupted: { label: "Interrupted", tone: "warning" },
};

export function jobState(state: string): Status {
  return statusOf(JOB_STATES, state);
}

export const TARGET_KINDS: readonly Choice<TargetKind>[] = [
  { id: "local_disk", label: "Second internal disk" },
  { id: "removable", label: "Removable drive" },
  { id: "network_share", label: "Network share" },
];

export const TARGET_PURPOSES: readonly Choice<TargetPurpose>[] = [
  { id: "backup_and_anchors", label: "Backups and audit anchors" },
  { id: "anchors_only", label: "Audit anchors only" },
];

const BACKUP_KINDS: Record<string, string> = {
  scheduled: "Scheduled",
  manual: "Manual",
  pre_update: "Before an update",
};

export function backupKind(kind: string): string {
  return BACKUP_KINDS[kind] ?? kind;
}

export const DISPOSAL_METHODS: readonly Choice<DisposalMethod>[] = [
  { id: "wiped", label: "Wiped" },
  { id: "destroyed", label: "Destroyed" },
  { id: "reused_after_wipe", label: "Reused after wiping" },
  { id: "returned", label: "Returned" },
];

/** Restore, restore-test and anchor findings (design brief §3.6 and §4). */
const FINDINGS: Record<string, string> = {
  manifest_unreadable: "The backup's contents list could not be read",
  key_mismatch: "The backup was made with a different installation key",
  database_digest_mismatch: "The database copy does not match its recorded fingerprint",
  integrity_failed: "The database copy failed its integrity check",
  schema_newer_than_app: "The backup is from a newer version of the app",
  open_failed: "The database copy could not be opened",
  chain_broken: "An audit trail chain is broken",
  head_mismatch: "An audit trail does not end where the backup recorded",
  seal_inconsistent: "An audit seal does not agree with its audit trail",
  anchor_mismatch: "An audit anchor does not match the audit trail",
  anchor_gap: "Audit anchors are missing between others",
  anchor_conflict: "Two different audit anchors exist for one seal",
  unverified_restore_boundary: "A restore boundary is not yet covered by an anchor",
  local_events_missing: "Audit events covered by an anchor are missing here",
  lost_interval: "Work recorded after the backup was made (expected after a restore)",
  artifact_missing: "A stored file is missing from the backup",
  artifact_corrupt: "A stored file in the backup is damaged",
  referenced_artifact_missing: "A file the records refer to is not in the backup",
};

export function findingText(code: string): string {
  return FINDINGS[code] ?? code;
}

/** The rows of an anchor comparison, in reading order. */
export const ANCHOR_COUNTS: readonly { key: keyof AnchorSummary; label: string }[] = [
  { key: "compared", label: "Anchors compared" },
  { key: "matched", label: "Matched" },
  { key: "lostInterval", label: "Lost interval (expected after a restore)" },
  { key: "unverifiedRestoreBoundary", label: "Explained by a restore not yet anchored" },
  { key: "mismatched", label: "Mismatched" },
  { key: "localEventsMissing", label: "Local events missing" },
  { key: "conflicts", label: "Conflicting anchors" },
  { key: "gaps", label: "Gaps" },
  { key: "unanchored", label: "Not yet anchored" },
];

const UNITS = ["bytes", "KiB", "MiB", "GiB", "TiB"];

/** A size for reading, such as `12.4 MiB`; `Unknown` when the engine sent none. */
export function sizeText(bytes: number | null): string {
  if (bytes === null) return "Unknown";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${bytes} bytes` : `${value.toFixed(1)} ${UNITS[unit]}`;
}
