import type { AnchorSummary } from "@/generated/AnchorSummary";
import type { BackupRunView } from "@/generated/BackupRunView";
import type { BackupSetView } from "@/generated/BackupSetView";
import type { BackupStatus } from "@/generated/BackupStatus";
import type { HealthCheckView } from "@/generated/HealthCheckView";
import type { HostHealthView } from "@/generated/HostHealthView";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { TargetView } from "@/generated/TargetView";

/** Synthetic operations records for the operations tests. No real laboratory data or paths. */

const ME = "00000000-0000-7000-8000-0000000000e1";
export const TARGET_ID = "00000000-0000-7000-8000-0000000000b1";
export const REMOVABLE_ID = "00000000-0000-7000-8000-0000000000b2";
export const RUN_ID = "00000000-0000-7000-8000-0000000000b3";
export const SET_ID = "20261013T140000Z-0000b3aa";

export function sessionWith(capabilities: string[]): SessionInfo {
  return {
    userId: ME,
    username: "operator",
    displayName: "Test Operator",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

export function target(overrides: Partial<TargetView> = {}): TargetView {
  return {
    targetId: TARGET_ID,
    label: "Synthetic network share",
    kind: "network_share",
    purpose: "backup_and_anchors",
    location: "\\\\synthetic-nas\\backups",
    available: true,
    lastVerifiedAt: "2026-10-13T14:00:00Z",
    lastFailure: null,
    retired: false,
    revision: 1,
    allowedActions: ["retire_backup_target", "request_restore_test"],
    ...overrides,
  };
}

export function backupRun(overrides: Partial<BackupRunView> = {}): BackupRunView {
  return {
    runId: RUN_ID,
    kind: "scheduled",
    state: "succeeded",
    requestedBy: null,
    requestedAt: "2026-10-13T14:00:00Z",
    startedAt: "2026-10-13T14:00:01Z",
    finishedAt: "2026-10-13T14:00:09Z",
    databaseBytes: 52_428_800,
    artifactCount: 12,
    artifactProblems: 0,
    failureCode: null,
    updateApprovalEligible: false,
    copies: [
      {
        targetId: TARGET_ID,
        targetLabel: "Synthetic network share",
        setId: SET_ID,
        state: "succeeded",
        failureCode: null,
        verifiedAt: "2026-10-13T14:00:09Z",
        pruned: false,
      },
    ],
    ...overrides,
  };
}

export function backupSet(overrides: Partial<BackupSetView> = {}): BackupSetView {
  return {
    setId: SET_ID,
    createdAt: "2026-10-13T14:00:00Z",
    kind: "scheduled",
    appVersion: "0.4.0",
    databaseBytes: 52_428_800,
    artifactCount: 12,
    compatible: true,
    incompatibleReason: null,
    ...overrides,
  };
}

export const BACKUP_STATUS: BackupStatus = {
  nextDueAt: "2026-10-13T14:15:00Z",
  running: null,
  newestVerified: { at: "2026-10-13T14:00:09Z", targetLabel: "Synthetic network share" },
  upToDate: true,
};

export const ANCHORS: AnchorSummary = {
  compared: 40,
  matched: 38,
  lostInterval: 2,
  unverifiedRestoreBoundary: 0,
  mismatched: 0,
  localEventsMissing: 0,
  conflicts: 0,
  gaps: 0,
  unanchored: 1,
};

export function healthCheck(overrides: Partial<HealthCheckView> = {}): HealthCheckView {
  return {
    check: "backup_age",
    label: "Backup age",
    state: "ok",
    detailCode: null,
    summary: "The newest verified copy covers every audit event.",
    value: "0 min",
    threshold: "Warning at 2 h, alert at 4 h",
    observedAt: "2026-10-13T14:05:00Z",
    ownerCapability: "backup_restore",
    nextAction: null,
    attestable: false,
    attestation: null,
    ...overrides,
  };
}

export function hostHealth(overrides: Partial<HostHealthView> = {}): HostHealthView {
  return {
    overall: "ok",
    checks: [healthCheck()],
    host: {
      os: "Synthetic OS",
      osVersion: "1.0",
      edition: null,
      arch: "x86_64",
      webview: "WebKitGTK 2.44",
      appVersion: "0.4.0",
      dataFolder: "/home/synthetic/.local/share/org.vibetox.lis",
    },
    keepAwake: { state: "not_needed", since: null, code: null },
    ...overrides,
  };
}
